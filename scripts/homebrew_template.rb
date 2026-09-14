class Pincer < Formula
  desc "High-Performance Rust Download Engine"
  homepage "https://github.com/grabbit/pincer-engine"
  
  # REPLACE THIS URL with the actual URL from your latest release
  url "https://github.com/grabbit/pincer-engine/releases/download/v1.0.0/pincer-macos.zip"
  
  # REPLACE THIS SHA256 with the actual checksum output by your GitHub Action release
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  
  version "1.0.0"

  def install
    bin.install "pincer"
  end

  test do
    system "#{bin}/pincer", "--help"
  end
end
