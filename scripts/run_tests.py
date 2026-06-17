#!/usr/bin/env python3
import subprocess
import os
import sys
import time
import shutil
import signal

def find_and_kill_pincer():
    print("Checking for existing running Pincer processes...")
    try:
        ps_out = subprocess.run(["ps", "aux"], capture_output=True, text=True)
        for line in ps_out.stdout.splitlines():
            # Strictly target the compiled Pincer binary paths to avoid killing IDE extensions 
            # like Antigravity which have 'pincer-engine' in their workspace path arguments.
            if ("target/debug/pincer" in line or "target/release/pincer" in line) and "grep" not in line:
                parts = line.split()
                if len(parts) >= 2:
                    pid = int(parts[1])
                    print(f"Killing conflicting Pincer process (PID: {pid})...")
                    try:
                        os.kill(pid, signal.SIGTERM)
                        time.sleep(1)
                    except Exception:
                        pass
    except Exception as e:
        print(f"Failed to check/kill running Pincer processes: {e}")

def main():
    # Find repository root (parent of tests folder)
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    os.chdir(repo_root)

    ci_mode = "--ci" in sys.argv

    print("=============================================")
    print("Pincer Test Runner: Preparing environment...")
    print("=============================================")

    # 1. Kill any port conflicts
    find_and_kill_pincer()
    
    # 2. Run CI Verification Checks (Short-Circuiting)
    print("\n=============================================")
    print("Running CI Verification Checks...")
    print("=============================================")
    
    print("1. Verifying formatting (cargo fmt --all -- --check)...")
    fmt_res = subprocess.run(["cargo", "fmt", "--all", "--", "--check"])
    if fmt_res.returncode != 0:
        print("Formatting issues found. Running auto-format (cargo fmt --all) automatically...")
        subprocess.run(["cargo", "fmt", "--all"])
        # Re-check formatting to be certain
        fmt_res = subprocess.run(["cargo", "fmt", "--all", "--", "--check"])
        if fmt_res.returncode != 0:
            print("\nError: Formatting check failed even after auto-format!")
            sys.exit(1)
        else:
            print("\x1b[32m✔ Code auto-formatted and verified successfully!\x1b[0m")
    else:
        print("\x1b[32m✔ Formatting verified successfully!\x1b[0m")
    
    print("\n2. Checking code compilation (cargo check --all-targets)...")
    check_res = subprocess.run(["cargo", "check", "--all-targets"])
    if check_res.returncode != 0:
        print("\nError: Code compilation check failed!")
        sys.exit(1)
    print("\x1b[32m✔ Code compilation verified successfully!\x1b[0m")
    
    print("\n3. Running Clippy linter (cargo clippy --all-targets)...")
    clippy_res = subprocess.run(["cargo", "clippy", "--all-targets", "--", "-D", "warnings"])
    if clippy_res.returncode != 0:
        print("\nError: Clippy linter check failed!")
        sys.exit(1)
    print("\x1b[32m✔ Clippy linter verified successfully!\x1b[0m")

    print("\n3.5 Running Rust tests (cargo test)...")
    test_res = subprocess.run(["cargo", "test"])
    if test_res.returncode != 0:
        print("\nError: Rust unit tests failed!")
        sys.exit(1)
    print("\x1b[32m✔ Rust unit tests verified successfully!\x1b[0m")

    # 3. Build the debug binary
    print("\n=============================================")
    print("Building Pincer debug binary...")
    print("=============================================")
    try:
        subprocess.run(["cargo", "build"], check=True)
        print("\x1b[32m✔ Debug binary built successfully!\x1b[0m")
    except subprocess.CalledProcessError:
        print("Error: Cargo build failed. Exiting.")
        sys.exit(1)

    # 4. Clear stale session files
    session_path = os.path.expanduser("~/.pincer/pincer.session")
    if os.path.exists(session_path):
        print("\nClearing stale session file...")
        try:
            os.remove(session_path)
            print("\x1b[32m✔ Session file cleared successfully!\x1b[0m")
        except OSError:
            pass

    # Ensure temporary directory exists
    temp_dir = os.path.join(repo_root, "tests", "temporary")
    os.makedirs(temp_dir, exist_ok=True)

    # 5. Run CLI Tests
    print("\n=============================================")
    print("Running CLI Tests...")
    print("=============================================")
    cli_test = subprocess.run([sys.executable, "-m", "unittest", "tests/test_cli.py", "-v"])
    cli_success = (cli_test.returncode == 0)
    if cli_success:
        print("\x1b[32m✔ CLI tests passed successfully!\x1b[0m")

    # 6. Start background server for RPC Tests
    print("\n=============================================")
    print("Starting RPC Server...")
    print("=============================================")
    server_proc = None
    try:
        # Start server in background
        server_proc = subprocess.Popen(
            ["./target/debug/pincer"], 
            stdout=subprocess.DEVNULL, 
            stderr=subprocess.DEVNULL
        )
        time.sleep(2)  # Give it time to spin up and bind the socket
        if server_proc.poll() is None:
            print("\x1b[32m✔ RPC server started successfully!\x1b[0m")
    except Exception as e:
        print(f"Failed to start RPC server: {e}")

    # 7. Run RPC Tests
    print("\n=============================================")
    print("Running RPC Tests...")
    print("=============================================")
    rpc_success = False
    if server_proc and server_proc.poll() is None:
        rpc_test = subprocess.run([sys.executable, "-m", "unittest", "tests/test_rpc.py", "-v"])
        rpc_success = (rpc_test.returncode == 0)
        if rpc_success:
            print("\x1b[32m✔ RPC tests passed successfully!\x1b[0m")
    else:
        print("Error: RPC server failed to start, skipping RPC tests.")

    # 7.5 Run Metalink Tests
    print("\n=============================================")
    print("Running Metalink Tests...")
    print("=============================================")
    metalink_success = False
    metalink_test = subprocess.run([sys.executable, "-m", "unittest", "tests/test_metalink.py", "-v"])
    metalink_success = (metalink_test.returncode == 0)
    if metalink_success:
        print("\x1b[32m✔ Metalink tests passed successfully!\x1b[0m")

    # 8. Shutdown background server
    if server_proc:
        print("\nShutting down RPC server...")
        try:
            server_proc.terminate()
            server_proc.wait(timeout=5)
        except Exception:
            try:
                server_proc.kill()
            except Exception:
                pass

    # 9. Report results and prompt for cleanup
    print("\n=============================================")
    print("Tests Summary:")
    print("  - Formatting (cargo fmt):     PASSED")
    print("  - Compilation (cargo check): PASSED")
    print("  - Lints (cargo clippy):      PASSED")
    print("  - Rust Tests (cargo test):   PASSED")
    print(f"  - CLI Tests:                 {'PASSED' if cli_success else 'FAILED'}")
    print(f"  - RPC Tests:                 {'PASSED' if rpc_success else 'FAILED'}")
    print(f"  - Metalink Tests:            {'PASSED' if metalink_success else 'FAILED'}")
    print("=============================================")
    
    if ci_mode:
        print("\nCI mode detected. Cleaning up temporary test files automatically...")
        if os.path.exists(temp_dir):
            shutil.rmtree(temp_dir, ignore_errors=True)
            print("Cleaned up tests/temporary successfully!")
    else:
        try:
            user_input = input("\nDo you want to clean up the temporary test files in tests/temporary? (Y/N): ").strip().lower()
            if user_input in ['y', 'yes']:
                print("Cleaning up temporary test files...")
                if os.path.exists(temp_dir):
                    shutil.rmtree(temp_dir, ignore_errors=True)
                    print("Cleaned up tests/temporary successfully!")
            else:
                print(f"Keeping temporary files in: {temp_dir}")
        except KeyboardInterrupt:
            print("\nSkipping cleanup.")

    # Exit code based on successes (prior stages must have passed to reach here)
    if cli_success and rpc_success and metalink_success:
        sys.exit(0)
    else:
        sys.exit(1)

if __name__ == "__main__":
    main()
