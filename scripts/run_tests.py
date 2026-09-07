#!/usr/bin/env python3
import subprocess
import os
import sys
import time
import shutil
import signal

def print_step(title, icon="⚙️"):
    print(f"\n\x1b[1;36m╭{'─' * 55}╮\x1b[0m")
    print(f"\x1b[1;36m│\x1b[0m {icon} \x1b[1;37m{title.ljust(50)}\x1b[1;36m│\x1b[0m")
    print(f"\x1b[1;36m╰{'─' * 55}╯\x1b[0m")

def print_summary(results):
    print(f"\n\x1b[1;35m╭{'─' * 55}╮\x1b[0m")
    print(f"\x1b[1;35m│\x1b[0m 📊 \x1b[1;37mTests Summary{' ' * 38}\x1b[1;35m│\x1b[0m")
    print(f"\x1b[1;35m├{'─' * 55}┤\x1b[0m")
    for name, success in results:
        status = "\x1b[1;32m✔ PASSED\x1b[0m" if success else "\x1b[1;31m✖ FAILED\x1b[0m"
        print(f"\x1b[1;35m│\x1b[0m  {name:<30} {status} {' ' * 13}\x1b[1;35m│\x1b[0m")
    print(f"\x1b[1;35m╰{'─' * 55}╯\x1b[0m\n")

def find_and_kill_pincer():
    print("\x1b[33mChecking for existing running Pincer processes...\x1b[0m")
    try:
        ps_out = subprocess.run(["ps", "aux"], capture_output=True, text=True)
        for line in ps_out.stdout.splitlines():
            # Strictly target the compiled Pincer binary paths to avoid killing IDE extensions 
            # like Antigravity which have 'pincer-engine' in their workspace path arguments.
            if ("target/debug/pincer" in line or "target/release/pincer" in line) and "grep" not in line:
                parts = line.split()
                if len(parts) >= 2:
                    pid = int(parts[1])
                    print(f"\x1b[31mKilling conflicting Pincer process (PID: {pid})...\x1b[0m")
                    try:
                        os.kill(pid, signal.SIGTERM)
                        time.sleep(1)
                    except Exception:
                        pass
    except Exception as e:
        print(f"\x1b[31mFailed to check/kill running Pincer processes: {e}\x1b[0m")

def main():
    # Find repository root (parent of tests folder)
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    os.chdir(repo_root)

    ci_mode = "--ci" in sys.argv

    print_step("Pincer Test Runner: Preparing environment...", "🚀")

    # 1. Kill any port conflicts
    find_and_kill_pincer()
    
    # 2. Run CI Verification Checks (Short-Circuiting)
    print_step("Running CI Verification Checks...", "🔍")
    
    print("\x1b[36m1. Verifying formatting (cargo fmt --all -- --check)...\x1b[0m")
    fmt_res = subprocess.run(["cargo", "fmt", "--all", "--", "--check"], capture_output=True, text=True)
    if fmt_res.returncode != 0:
        print("\x1b[33mFormatting issues found. Running auto-format (cargo fmt --all) automatically...\x1b[0m")
        subprocess.run(["cargo", "fmt", "--all"], capture_output=True, text=True)
        # Re-check formatting to be certain
        fmt_res = subprocess.run(["cargo", "fmt", "--all", "--", "--check"], capture_output=True, text=True)
        if fmt_res.returncode != 0:
            print(fmt_res.stdout)
            print(fmt_res.stderr)
            print("\n\x1b[1;31m✖ Error: Formatting check failed even after auto-format!\x1b[0m")
            sys.exit(1)
        else:
            print("\x1b[1;32m✔ Code auto-formatted and verified successfully!\x1b[0m")
    else:
        print("\x1b[1;32m✔ Formatting verified successfully!\x1b[0m")
    
    print("\n\x1b[36m2. Checking code compilation (cargo check --all-targets)...\x1b[0m")
    check_res = subprocess.run(["cargo", "check", "--all-targets"], capture_output=True, text=True)
    if check_res.returncode != 0:
        print(check_res.stdout)
        print(check_res.stderr)
        print("\n\x1b[1;31m✖ Error: Code compilation check failed!\x1b[0m")
        sys.exit(1)
    print("\x1b[1;32m✔ Code compilation verified successfully!\x1b[0m")
    
    print("\n\x1b[36m3. Running Clippy linter (cargo clippy --all-targets)...\x1b[0m")
    clippy_res = subprocess.run(["cargo", "clippy", "--all-targets", "--", "-D", "warnings"], capture_output=True, text=True)
    if clippy_res.returncode != 0:
        print(clippy_res.stdout)
        print(clippy_res.stderr)
        print("\n\x1b[1;31m✖ Error: Clippy linter check failed!\x1b[0m")
        sys.exit(1)
    print("\x1b[1;32m✔ Clippy linter verified successfully!\x1b[0m")

    print("\n\x1b[36m3.5 Running Rust tests (cargo test)...\x1b[0m")
    test_res = subprocess.run(["cargo", "test"], capture_output=True, text=True)
    if test_res.returncode != 0:
        print(test_res.stdout)
        print(test_res.stderr)
        print("\n\x1b[1;31m✖ Error: Rust unit tests failed!\x1b[0m")
        sys.exit(1)
    print("\x1b[1;32m✔ Rust unit tests verified successfully!\x1b[0m")

    # 3. Build the debug binary
    print_step("Building Pincer debug binary...", "🔨")
    try:
        subprocess.run(["cargo", "build"], capture_output=True, text=True, check=True)
        print("\x1b[1;32m✔ Debug binary built successfully!\x1b[0m")
    except subprocess.CalledProcessError as e:
        print(e.stdout)
        print(e.stderr)
        print("\x1b[1;31m✖ Error: Cargo build failed. Exiting.\x1b[0m")
        sys.exit(1)

    # 4. Clear stale session files
    session_path = os.path.expanduser("~/.pincer/pincer.session")
    if os.path.exists(session_path):
        print("\n\x1b[33mClearing stale session file...\x1b[0m")
        try:
            os.remove(session_path)
            print("\x1b[1;32m✔ Session file cleared successfully!\x1b[0m")
        except OSError:
            pass

    # Ensure temporary directories exist
    temp_dirs = [
        os.path.join(repo_root, "tests", "temporary"),
        os.path.join(repo_root, "tests", "cli", "temporary"),
        os.path.join(repo_root, "tests", "core", "temporary"),
        os.path.join(repo_root, "tests", "rpc", "temporary"),
    ]
    for td in temp_dirs:
        os.makedirs(td, exist_ok=True)

    # 5. Run CLI Tests
    print_step("Running CLI Tests...", "💻")
    cli_test = subprocess.run([sys.executable, "-m", "unittest", "discover", "-s", "tests/cli", "-v"])
    cli_success = (cli_test.returncode == 0)
    if cli_success:
        print("\n\x1b[1;32m✔ CLI tests passed successfully!\x1b[0m")

    # 6. Start background server for RPC Tests
    print_step("Starting RPC Server...", "🔌")
    server_proc = None
    try:
        # Start server in background
        server_proc = subprocess.Popen(
            ["./target/debug/pincer", "--enable-rpc", "--rpc-listen-port", "6842"], 
            stdout=subprocess.DEVNULL, 
            stderr=subprocess.DEVNULL
        )
        time.sleep(2)  # Give it time to spin up and bind the socket
        if server_proc.poll() is None:
            print("\x1b[1;32m✔ RPC server started successfully!\x1b[0m")
    except Exception as e:
        print(f"\x1b[1;31m✖ Failed to start RPC server: {e}\x1b[0m")

    # 7. Run RPC Tests
    print_step("Running RPC Tests...", "📡")
    rpc_success = False
    if server_proc and server_proc.poll() is None:
        rpc_test = subprocess.run([sys.executable, "-m", "unittest", "discover", "-s", "tests/rpc", "-v"])
        rpc_success = (rpc_test.returncode == 0)
        if rpc_success:
            print("\n\x1b[1;32m✔ RPC tests passed successfully!\x1b[0m")
    else:
        print("\x1b[1;31m✖ Error: RPC server failed to start, skipping RPC tests.\x1b[0m")

    # 7.5 Run Metalink Tests
    print_step("Running Metalink Tests...", "🔗")
    metalink_success = False
    if server_proc and server_proc.poll() is None:
        metalink_test = subprocess.run([sys.executable, "-m", "unittest", "discover", "-s", "tests/core", "-p", "test_metalink.py", "-v"])
        metalink_success = (metalink_test.returncode == 0)
        if metalink_success:
            print("\n\x1b[1;32m✔ Metalink tests passed successfully!\x1b[0m")
    else:
        print("\x1b[1;31m✖ Error: RPC server failed to start, skipping Metalink tests.\x1b[0m")

    # 7.6 Run Core Fixes Tests
    print_step("Running Core Fixes Tests...", "🛠️")
    core_fixes_success = False
    if server_proc and server_proc.poll() is None:
        core_fixes_test = subprocess.run([sys.executable, "-m", "unittest", "discover", "-s", "tests/core", "-p", "test_core_fixes.py", "-v"])
        core_fixes_success = (core_fixes_test.returncode == 0)
        if core_fixes_success:
            print("\n\x1b[1;32m✔ Core Fixes tests passed successfully!\x1b[0m")
    else:
        print("\x1b[1;31m✖ Error: RPC server failed to start, skipping Core Fixes tests.\x1b[0m")

    # 8. Shutdown background server
    if server_proc:
        print("\n\x1b[33mShutting down RPC server...\x1b[0m")
        try:
            server_proc.terminate()
            server_proc.wait(timeout=5)
        except Exception:
            try:
                server_proc.kill()
            except Exception:
                pass

    # 9. Report results and prompt for cleanup
    results = [
        ("Formatting (cargo fmt)", True),
        ("Compilation (cargo check)", True),
        ("Lints (cargo clippy)", True),
        ("Rust Tests (cargo test)", True),
        ("CLI Tests", cli_success),
        ("RPC Tests", rpc_success),
        ("Metalink Tests", metalink_success),
        ("Core Fixes Tests", core_fixes_success),
    ]
    print_summary(results)
    
    if ci_mode:
        print("\x1b[36mCI mode detected. Cleaning up temporary test files automatically...\x1b[0m")
        for td in temp_dirs:
            if os.path.exists(td):
                shutil.rmtree(td, ignore_errors=True)
        print("\x1b[1;32m✔ Cleaned up all temporary directories successfully!\x1b[0m")
    else:
        try:
            user_input = input("\x1b[1;33mDo you want to clean up all temporary test files? (Y/N):\x1b[0m ").strip().lower()
            if user_input in ['y', 'yes']:
                print("\x1b[36mCleaning up temporary test files...\x1b[0m")
                for td in temp_dirs:
                    if os.path.exists(td):
                        shutil.rmtree(td, ignore_errors=True)
                print("\x1b[1;32m✔ Cleaned up all temporary directories successfully!\x1b[0m")
            else:
                print("\x1b[36mKeeping temporary files.\x1b[0m")
        except KeyboardInterrupt:
            print("\n\x1b[33mSkipping cleanup.\x1b[0m")

    # Exit code based on successes (prior stages must have passed to reach here)
    if cli_success and rpc_success and metalink_success and core_fixes_success:
        sys.exit(0)
    else:
        sys.exit(1)

if __name__ == "__main__":
    main()
