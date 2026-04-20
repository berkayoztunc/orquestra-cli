class OrquestraCli < Formula
  desc "CLI for interacting with Solana programs via orquestra.dev"
  homepage "https://github.com/berkayoztunc/orquestra-cli"
  version "0.2.5"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/berkayoztunc/orquestra-cli/releases/download/v#{version}/orquestra-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "e1f464b647b3d723a4887cff5f531e9861b31af0881b87fc7a18415a33c9242a"
    else
      url "https://github.com/berkayoztunc/orquestra-cli/releases/download/v#{version}/orquestra-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "b3cce9662f52af1acacde83f834a82ca4d7dd56e8054dcfe8273ec310eb787bb"
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
