class OrquestraCli < Formula
  desc "CLI for interacting with Solana programs via orquestra.dev"
  homepage "https://github.com/berkayoztunc/orquestra-cli"
  version "0.2.1"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/berkayoztunc/orquestra-cli/releases/download/v#{version}/orquestra-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "10b8877562fbca2b39432c78af880531fd0a46f2e1b6877012893b853680cfdb"
    else
      url "https://github.com/berkayoztunc/orquestra-cli/releases/download/v#{version}/orquestra-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "73db3b3d8ff96aba93ced5b13f57908a73bec53f421b8b1f40d3780ead300c76"
    end
  end

  def install
    bin.install "orquestra"
  end

  def caveats
    <<~EOS
      After installing, configure orquestra for your project:

        orquestra config set \\
          --project-id <your-project-id> \\
          --api-key <your-api-key> \\
          --rpc https://api.mainnet-beta.solana.com \\
          --keypair ~/.config/solana/id.json

      Then list instructions:
        orquestra list

      Or run an instruction interactively:
        orquestra run
    EOS
  end

  test do
    assert_match "orquestra", shell_output("#{bin}/orquestra --version")
  end
end
