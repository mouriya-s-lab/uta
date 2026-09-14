import { z } from 'zod';
import { accountIdSchema,entryPositionSchema,sha256Schema } from '../ids.ts';
import type { AccountId,LedgerPosition,HeadPosition,EntryPosition,Instant,Sha256,SegmentName,ByteOffset } from '../ids.ts';
import { ledgerEntrySchema,unknownLedgerEntrySchema } from './entries.ts';
import type { LedgerEntry,UnknownLedgerEntry } from './entries.ts';
import type { ParseError,Result } from '../result.ts';
export const GENESIS_HASH=sha256Schema.parse('0'.repeat(64));
export type EntryDraft=LedgerEntry extends infer E?E extends LedgerEntry?Omit<E,'position'|'recordedAt'>:never:never;
export type Durable={readonly kind:'Durable'};
export type DurableWeak={readonly kind:'DurableWeak';readonly platform:string;readonly reason:string};
export type DurabilityResult=Durable|DurableWeak;
export type AppendResult={readonly kind:'Appended';readonly position:EntryPosition;readonly durability:DurabilityResult}|{readonly kind:'Conflict';readonly expected:LedgerPosition;readonly actual:HeadPosition}|{readonly kind:'DurabilityFailure';readonly result:{readonly stage:'segmentSync'|'headReplace'|'directorySync';readonly reason:ParseError}}|{readonly kind:'closed'};
export type CloseResult={readonly kind:'Closed'}|{readonly kind:'CloseFailure';readonly reason:ParseError};
export type HeadState={readonly schemaVersion:2;readonly accountId:AccountId;readonly position:HeadPosition;readonly hash:Sha256;readonly segment:SegmentName;readonly byteOffset:ByteOffset;readonly updatedAt:Instant};
export const ledgerFrameSchema=z.strictObject({schemaVersion:z.literal(2),accountId:accountIdSchema,position:entryPositionSchema,entries:z.array(z.union([ledgerEntrySchema,unknownLedgerEntrySchema])).min(1),prevHash:sha256Schema}).superRefine((frame,context)=>{
 const last=frame.entries.at(-1);if(last?.position!==frame.position)context.addIssue({code:'custom',message:'Frame position is not its final entry'});
 for(let index=0;index<frame.entries.length;index++){const entry=frame.entries[index];if(!entry)continue;const prior=frame.entries[index-1];if(entry.accountId!==frame.accountId||(prior&&entry.position!==prior.position+1))context.addIssue({code:'custom',message:'Frame account or contiguous position mismatch'});
  if(entry.kind!=='recovery.resolved')continue;
  const resolved=ledgerEntrySchema.safeParse(entry);if(!resolved.success||resolved.data.kind!=='recovery.resolved'||resolved.data.payload.result.kind!=='found')continue;
  const result=resolved.data.payload.result,receipt=frame.entries.find(candidate=>candidate.entryId===result.receiptEntryId),observation=frame.entries.find(candidate=>candidate.entryId===result.observationEntryId);
  const r=ledgerEntrySchema.safeParse(receipt),o=ledgerEntrySchema.safeParse(observation);
  if(!r.success||r.data.kind!=='receipt.recorded'||r.data.payload.receipt.payload.kind!=='accepted'||r.data.payload.receipt.payload.provenance.kind!=='recovered'||!o.success||o.data.kind!=='observation.recorded'||r.data.payload.intentId!==resolved.data.payload.intentId||r.data.payload.attemptNo!==resolved.data.payload.attemptNo||o.data.correlation.intentId!==resolved.data.payload.intentId||o.data.correlation.attemptNo!==resolved.data.payload.attemptNo||o.data.payload.observation.payload.providerRef!==r.data.payload.receipt.payload.providerRef||o.data.payload.observation.providerId!==r.data.payload.receipt.providerId||o.data.payload.observation.projectionVersion!==r.data.payload.receipt.projectionVersion)context.addIssue({code:'custom',message:'Recovered references must name matching receipt and observation in the same frame'});
 }
});
export type LedgerFrame=z.output<typeof ledgerFrameSchema>;
export function parseLedgerFrame(input:unknown):Result<LedgerFrame,{readonly kind:'Corrupt';readonly reason:ParseError}>{const parsed=ledgerFrameSchema.safeParse(input);return parsed.success?{kind:'ok',value:parsed.data}:{kind:'error',error:{kind:'Corrupt',reason:{code:'InvalidInput',path:'LedgerFrame',message:parsed.error.message}}};}
export type RecoveryReport={readonly accountId:AccountId;readonly segment:SegmentName;readonly reason:ParseError;readonly headPosition:HeadPosition;readonly headHash:Sha256}&({readonly kind:'TornTail'|'UncommittedTail';readonly quarantinePath:string;readonly sourceHash:Sha256}|{readonly kind:'Corrupt';readonly position?:EntryPosition});
export type OpenResult={readonly kind:'Open';readonly head:HeadState}|{readonly kind:'TornTail';readonly quarantined:readonly RecoveryReport[];readonly head:HeadState}|{readonly kind:'Corrupt';readonly at:RecoveryReport};
export interface AppendStore{open():Promise<OpenResult>;append(entries:readonly [EntryDraft,...EntryDraft[]],expectedPosition:LedgerPosition):Promise<AppendResult>;replay(fromPosition:LedgerPosition):AsyncIterable<LedgerEntry|UnknownLedgerEntry>;head():Promise<HeadState>;close():Promise<CloseResult>;}
export type ViewCheckpoint={readonly schemaVersion:1;readonly viewId:string;readonly sourcePosition:HeadPosition;readonly sourceHash:Sha256;readonly updatedAt:Instant};
export type MigrationMarker={readonly kind:'archived';readonly legacyDigest:Sha256;readonly archivedPath:'legacy/commit.json';readonly completedAt:Instant}|{readonly kind:'noLegacySource';readonly completedAt:Instant};
