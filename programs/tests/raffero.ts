import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { Raffero } from "../target/types/raffero";

describe("raffero", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.raffero as Program<Raffero>;

  it("Is initialized!", async () => {
    // Add your test here.
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  });
});
