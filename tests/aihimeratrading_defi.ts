import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { expect } from "chai";
import {
  createMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  getAccount,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { PublicKey, SystemProgram } from "@solana/web3.js";

describe("aihimeratrading_defi", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.AihimeratradingDefi as Program;
  const authority = provider.wallet as anchor.Wallet;
  const creator = authority;
  const staker = authority;

  let mint: PublicKey;
  let creatorAta: PublicKey;
  let protocol: PublicKey;
  let treasury: PublicKey;
  let signal: PublicKey;
  let signalVault: PublicKey;
  let position: PublicKey;

  it("initializes protocol, mints reward/stake token, creates treasury PDA", async () => {
    mint = await createMint(
      provider.connection,
      authority.payer,
      authority.publicKey,
      null,
      6
    );

    const ata = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      authority.payer,
      mint,
      authority.publicKey
    );
    creatorAta = ata.address;

    await mintTo(
      provider.connection,
      authority.payer,
      mint,
      creatorAta,
      authority.publicKey,
      10_000_000_000
    );

    [protocol] = PublicKey.findProgramAddressSync(
      [Buffer.from("protocol"), authority.publicKey.toBuffer(), mint.toBuffer()],
      program.programId
    );
    [treasury] = PublicKey.findProgramAddressSync(
      [Buffer.from("treasury"), protocol.toBuffer()],
      program.programId
    );

    await program.methods
      .initializeProtocol(1_000, 100) // 10% reward-rate multiplier, 1% platform fee
      .accounts({
        authority: authority.publicKey,
        mint,
        protocol,
        treasury,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    await mintTo(
      provider.connection,
      authority.payer,
      mint,
      treasury,
      authority.publicKey,
      1_000_000_000
    );

    const protocolAccount = await program.account.protocol.fetch(protocol);
    expect(protocolAccount.signalCount.toNumber()).to.equal(0);
  });

  it("creates an AI trading signal with initial stake", async () => {
    const protocolAccount = await program.account.protocol.fetch(protocol);
    const id = protocolAccount.signalCount.toNumber();

    [signal] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("signal"),
        protocol.toBuffer(),
        creator.publicKey.toBuffer(),
        new anchor.BN(id).toArrayLike(Buffer, "le", 8),
      ],
      program.programId
    );
    [signalVault] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), signal.toBuffer()],
      program.programId
    );

    await program.methods
      .createSignal(
        "SOL",
        "ipfs://aihimeratrading/sol-long-v1",
        "sha256:ai-himera-rationale-hash",
        { long: {} },
        8_500,
        new anchor.BN(1_000_000)
      )
      .accounts({
        creator: creator.publicKey,
        mint,
        protocol,
        signal,
        signalVault,
        creatorTokenAccount: creatorAta,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    const signalAccount = await program.account.signal.fetch(signal);
    expect(signalAccount.symbol).to.equal("SOL");
    expect(signalAccount.totalStaked.toNumber()).to.equal(1_000_000);
    expect(signalAccount.isActive).to.equal(true);
  });

  it("updates signal metadata and stakes more tokens", async () => {
    await program.methods
      .updateSignal(
        "ipfs://aihimeratrading/sol-long-v2",
        "sha256:updated-rationale-hash",
        { long: {} },
        9_100
      )
      .accounts({ creator: creator.publicKey, signal })
      .rpc();

    [position] = PublicKey.findProgramAddressSync(
      [Buffer.from("position"), signal.toBuffer(), staker.publicKey.toBuffer()],
      program.programId
    );

    await program.methods
      .stakeSignal(new anchor.BN(2_000_000))
      .accounts({
        staker: staker.publicKey,
        mint,
        signal,
        signalVault,
        position,
        stakerTokenAccount: creatorAta,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const positionAccount = await program.account.userPosition.fetch(position);
    expect(positionAccount.stakedAmount.toNumber()).to.equal(2_000_000);
  });

  it("scores performance, claims rewards, deactivates signal, and withdraws stake", async () => {
    await program.methods
      .scoreSignal(2_500) // +25% performance score
      .accounts({ authority: authority.publicKey, protocol, signal })
      .rpc();

    const before = await getAccount(provider.connection, creatorAta);

    await program.methods
      .claimRewards()
      .accounts({
        staker: staker.publicKey,
        mint,
        protocol,
        signal,
        position,
        owner: staker.publicKey,
        treasury,
        stakerTokenAccount: creatorAta,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    const afterClaim = await getAccount(provider.connection, creatorAta);
    expect(Number(afterClaim.amount)).to.be.greaterThan(Number(before.amount));

    await program.methods
      .deactivateSignal()
      .accounts({ creator: creator.publicKey, signal })
      .rpc();

    await program.methods
      .withdrawStake(new anchor.BN(1_000_000))
      .accounts({
        staker: staker.publicKey,
        mint,
        signal,
        signalVault,
        position,
        owner: staker.publicKey,
        stakerTokenAccount: creatorAta,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    const positionAccount = await program.account.userPosition.fetch(position);
    expect(positionAccount.stakedAmount.toNumber()).to.equal(1_000_000);
  });
});
