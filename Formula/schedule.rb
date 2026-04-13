class Schedule < Formula
  desc "Natural language job scheduler"
  homepage "https://github.com/mnishizawa/scheduler"
  url "https://github.com/mnishizawa/scheduler.git", branch: "main"
  version "0.1.0"
  license "GPL-3.0-or-later"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    # Verify that the --version flag works
    output = shell_output("#{bin}/schedule --version")
    assert_match "schedule 0.1.0", output
  end
end
