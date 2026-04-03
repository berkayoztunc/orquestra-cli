class OrquestraCli < Formula
  desc "CLI for interacting with Solana programs via orquestra.dev"
  homepage "https://github.com/berkayoztunc/orquestra-cli"
  version "0.1.1"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/berkayoztunc/orquestra-cli/releases/download/v#{version}/orquestra-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "0ff6482e4a015ee17f5e122d1fc6a93ded518042c2646497ba9db15e6f870fed"
    else
      url "https://github.com/berkayoztunc/orquestra-cli/releases/download/v#{version}/orquestra-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "1f7559467f11589b4a7b5b253b28a0ab942de750d23a106706ff83f0db8ef617"
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
