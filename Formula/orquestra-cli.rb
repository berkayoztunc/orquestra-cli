class OrquestraCli < Formula
  desc "CLI for interacting with Solana programs via orquestra.dev"
  homepage "https://github.com/berkayoztunc/orquestra-cli"
  version "0.2.4"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/berkayoztunc/orquestra-cli/releases/download/v#{version}/orquestra-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "ff359ea6d1e318c8fdbce564d38b625f53fd1cf77155a8c6b3d30d3913b8503b"
    else
      url "https://github.com/berkayoztunc/orquestra-cli/releases/download/v#{version}/orquestra-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "27711d4c035c912676c0fe574d7ddc052e6063c3f771c22577e2ed4f274b783a"
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
