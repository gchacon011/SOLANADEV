import * as anchor from "@coral-xyz/anchor";
import { Program, AnchorProvider, BN } from "@coral-xyz/anchor";
import { PublicKey, SystemProgram } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";

export type TradeDirection = "long" | "short" | "neutral";

export function directionToAnchor(direction: TradeDirection) {
  if (direction === "long") return { long: {} };
  if (direction === "short") return { short: {} };
  return { neutral: {} };
}

export function deriveProtocol(programId: PublicKey, authority: PublicKey, mint: PublicKey) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("protocol"), authority.toBuffer(), mint.toBuffer()],
    programId
  );
}

export function deriveTreasury(programId: PublicKey, protocol: PublicKey) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("treasury"), protocol.toBuffer()],
    programId
  );
}

export function deriveSignal(
  programId: PublicKey,
  protocol: PublicKey,
  creator: PublicKey,
  signalId: number | BN
) {
  const id = BN.isBN(signalId) ? signalId : new BN(signalId);
  return PublicKey.findProgramAddressSync(
    [Buffer.from("signal"), protocol.toBuffer(), creator.toBuffer(), id.toArrayLike(Buffer, "le", 8)],
    programId
  );
}

export function deriveSignalVault(programId: PublicKey, signal: PublicKey) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), signal.toBuffer()],
    programId
  );
}

export function deriveUserPosition(programId: PublicKey, signal: PublicKey, user: PublicKey) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("position"), signal.toBuffer(), user.toBuffer()],
    programId
  );
}

export async function createSignalTx(params: {
  provider: AnchorProvider;
  program: Program;
  mint: PublicKey;
  protocol: PublicKey;
  creatorTokenAccount: PublicKey;
  symbol: string;
  strategyUri: string;
  rationaleHash: string;
  direction: TradeDirection;
  confidenceBps: number;
  initialStake: BN;
}) {
  const { provider, program } = params;
  const protocolAccount: any = await program.account.protocol.fetch(params.protocol);
  const [signal] = deriveSignal(
    program.programId,
    params.protocol,
    provider.wallet.publicKey,
    protocolAccount.signalCount
  );
  const [signalVault] = deriveSignalVault(program.programId, signal);

  return program.methods
    .createSignal(
      params.symbol,
      params.strategyUri,
      params.rationaleHash,
      directionToAnchor(params.direction),
      params.confidenceBps,
      params.initialStake
    )
    .accounts({
      creator: provider.wallet.publicKey,
      mint: params.mint,
      protocol: params.protocol,
      signal,
      signalVault,
      creatorTokenAccount: params.creatorTokenAccount,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      rent: anchor.web3.SYSVAR_RENT_PUBKEY,
    })
    .rpc();
}

export async function stakeSignalTx(params: {
  provider: AnchorProvider;
  program: Program;
  mint: PublicKey;
  signal: PublicKey;
  signalVault: PublicKey;
  stakerTokenAccount: PublicKey;
  amount: BN;
}) {
  const [position] = deriveUserPosition(
    params.program.programId,
    params.signal,
    params.provider.wallet.publicKey
  );

  return params.program.methods
    .stakeSignal(params.amount)
    .accounts({
      staker: params.provider.wallet.publicKey,
      mint: params.mint,
      signal: params.signal,
      signalVault: params.signalVault,
      position,
      stakerTokenAccount: params.stakerTokenAccount,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .rpc();
}
