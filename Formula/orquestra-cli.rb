class OrquestraCli < Formula
  desc "CLI for interacting with Solana programs via orquestra.dev"
  homepage "https://github.com/berkayoztunc/orquestra-cli"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/berkayoztunc/orquestra-cli/releases/download/v#{version}/orquestra-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "0d48dfeb7934ba24d559509472bf79dcade4c3fb19f32f347fd538b5a5574be8"
    else
      url "https://github.com/berkayoztunc/orquestra-cli/releases/download/v#{version}/orquestra-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "d3531ca1772b17ef47c8acd33617c2a40022f927ea005463461ccd8ae2f75f4c"
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
