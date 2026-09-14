import { z } from 'zod';
import type { Result, ParseError } from './result.ts';
declare const brand:unique symbol;
declare const positionClass:unique symbol;
export type Brand<T,Name> = T & {readonly [brand]:Name};
export function schemaFromParser<T>(parser:(input:unknown)=>Result<T,ParseError>) {
 return z.unknown().transform((input,context)=>{const parsed=parser(input);if(parsed.kind==='ok')return parsed.value;context.addIssue({code:'custom',message:parsed.error.message});return z.NEVER;});
}
function failure(path:string,message:string):Result<never,ParseError>{return {kind:'error',error:{code:'InvalidInput',path,message}};}
// PARSER BOUNDARY: erased brands attach only after grammar/range verification.
function text<N extends string>(input:unknown,name:N,pattern:RegExp=/^[^\s\x00-\x1f\x7f]+$/):Result<Brand<string,N>,ParseError>{
 if(typeof input!=='string'||input.length===0||input.length>512||!pattern.test(input))return failure(name,'Invalid identifier representation');
 return {kind:'ok',value:input as Brand<string,N>};
}
function integer<N extends string>(input:unknown,name:N,min:number):Result<Brand<number,N>,ParseError>{
 if(typeof input!=='number'||!Number.isSafeInteger(input)||input<min)return failure(name,'Expected safe integer in range');
 return {kind:'ok',value:input as Brand<number,N>};
}
export type AccountId=Brand<string,'AccountId'>;
export function parseAccountId(input:unknown):Result<AccountId,ParseError>{return text(input,'AccountId',/^[A-Za-z0-9][A-Za-z0-9_-]*$/);}
export const accountIdSchema=schemaFromParser(parseAccountId);
export type ProviderId=Brand<string,'ProviderId'>;
export function parseProviderId<const S extends string>(input:S):Result<Brand<S,'ProviderId'>,ParseError>;
export function parseProviderId(input:unknown):Result<ProviderId,ParseError>;
export function parseProviderId(input:unknown):Result<ProviderId,ParseError>{return text(input,'ProviderId');}
export const providerIdSchema=schemaFromParser(parseProviderId);
export type ProjectionVersion=Brand<string,'ProjectionVersion'>;
export function parseProjectionVersion<const S extends string>(input:S):Result<Brand<S,'ProjectionVersion'>,ParseError>;
export function parseProjectionVersion(input:unknown):Result<ProjectionVersion,ParseError>;
export function parseProjectionVersion(input:unknown):Result<ProjectionVersion,ParseError>{return text(input,'ProjectionVersion');}
export const projectionVersionSchema=schemaFromParser(parseProjectionVersion);
export type IntentId=Brand<string,'IntentId'>;
export function parseIntentId(input:unknown):Result<IntentId,ParseError>{return text(input,'IntentId');}
export const intentIdSchema=schemaFromParser(parseIntentId);
export type ProposalId=Brand<string,'ProposalId'>;
export function parseProposalId(input:unknown):Result<ProposalId,ParseError>{return text(input,'ProposalId');}
export const proposalIdSchema=schemaFromParser(parseProposalId);
export type DecisionId=Brand<string,'DecisionId'>;
export function parseDecisionId(input:unknown):Result<DecisionId,ParseError>{return text(input,'DecisionId');}
export const decisionIdSchema=schemaFromParser(parseDecisionId);
export type EntryId=Brand<string,'EntryId'>;
export function parseEntryId(input:unknown):Result<EntryId,ParseError>{return text(input,'EntryId');}
export const entryIdSchema=schemaFromParser(parseEntryId);
export type IdempotencyKey=Brand<string,'IdempotencyKey'>;
export function parseIdempotencyKey(input:unknown):Result<IdempotencyKey,ParseError>{return text(input,'IdempotencyKey');}
export const idempotencyKeySchema=schemaFromParser(parseIdempotencyKey);
export type AliceId=Brand<string,'AliceId'>;
export function parseAliceId(input:unknown):Result<AliceId,ParseError>{return text(input,'AliceId');}
export const aliceIdSchema=schemaFromParser(parseAliceId);
export type NativeKey=Brand<string,'NativeKey'>;
export function parseNativeKey(input:unknown):Result<NativeKey,ParseError>{return text(input,'NativeKey');}
export const nativeKeySchema=schemaFromParser(parseNativeKey);
export type RequestId=Brand<string,'RequestId'>;
export function parseRequestId(input:unknown):Result<RequestId,ParseError>{return text(input,'RequestId');}
export const requestIdSchema=schemaFromParser(parseRequestId);
export type Source=Brand<string,'Source'>;
export function parseSource(input:unknown):Result<Source,ParseError>{return text(input,'Source');}
export const sourceSchema=schemaFromParser(parseSource);
export type ProviderErrorCode=Brand<string,'ProviderErrorCode'>;
export function parseProviderErrorCode(input:unknown):Result<ProviderErrorCode,ParseError>{return text(input,'ProviderErrorCode');}
export const providerErrorCodeSchema=schemaFromParser(parseProviderErrorCode);
export type ConfigRevision=Brand<string,'ConfigRevision'>;
export function parseConfigRevision(input:unknown):Result<ConfigRevision,ParseError>{return text(input,'ConfigRevision',/^[a-f0-9]{64}$/);}
export const configRevisionSchema=schemaFromParser(parseConfigRevision);
export type ScopeHash=Brand<string,'ScopeHash'>;
export function parseScopeHash(input:unknown):Result<ScopeHash,ParseError>{return text(input,'ScopeHash',/^[a-f0-9]{64}$/);}
export const scopeHashSchema=schemaFromParser(parseScopeHash);
export type IntentHash=Brand<string,'IntentHash'>;
export function parseIntentHash(input:unknown):Result<IntentHash,ParseError>{return text(input,'IntentHash',/^[a-f0-9]{64}$/);}
export const intentHashSchema=schemaFromParser(parseIntentHash);
export type Sha256=Brand<string,'Sha256'>;
export function parseSha256(input:unknown):Result<Sha256,ParseError>{return text(input,'Sha256',/^[a-f0-9]{64}$/);}
export const sha256Schema=schemaFromParser(parseSha256);
export type SourceDigest=Brand<string,'SourceDigest'>;
export function parseSourceDigest(input:unknown):Result<SourceDigest,ParseError>{return text(input,'SourceDigest',/^[a-f0-9]{64}$/);}
export const sourceDigestSchema=schemaFromParser(parseSourceDigest);
export type AttemptNo=Brand<number,'AttemptNo'>;
export function parseAttemptNo(input:unknown):Result<AttemptNo,ParseError>{return integer(input,'AttemptNo',1);}
export const attemptNoSchema=schemaFromParser(parseAttemptNo);
export type LedgerPosition=Brand<number,'LedgerPosition'>;
export function parseLedgerPosition(input:unknown):Result<LedgerPosition,ParseError>{return integer(input,'LedgerPosition',0);}
export const ledgerPositionSchema=schemaFromParser(parseLedgerPosition);
export type Instant=Brand<number,'Instant'>;
export function parseInstant(input:unknown):Result<Instant,ParseError>{return integer(input,'Instant',0);}
export const instantSchema=schemaFromParser(parseInstant);
export type AsOf=Brand<number,'AsOf'>;
export function parseAsOf(input:unknown):Result<AsOf,ParseError>{return integer(input,'AsOf',0);}
export const asOfSchema=schemaFromParser(parseAsOf);
export type ByteOffset=Brand<number,'ByteOffset'>;
export function parseByteOffset(input:unknown):Result<ByteOffset,ParseError>{return integer(input,'ByteOffset',0);}
export const byteOffsetSchema=schemaFromParser(parseByteOffset);
export type EntryPosition=LedgerPosition & {readonly [positionClass]:'entry'};
export type HeadPosition=LedgerPosition & {readonly [positionClass]:'head'};
export function parseEntryPosition(input:unknown):Result<EntryPosition,ParseError>{const parsed=integer(input,'LedgerPosition',1);return parsed.kind==='error'?parsed:{kind:'ok',value:parsed.value as EntryPosition};}
export function parseHeadPosition(input:unknown):Result<HeadPosition,ParseError>{const parsed=parseLedgerPosition(input);return parsed.kind==='error'?parsed:{kind:'ok',value:parsed.value as HeadPosition};}
export const entryPositionSchema=schemaFromParser(parseEntryPosition);
export const headPositionSchema=schemaFromParser(parseHeadPosition);
export type SegmentName=Brand<string,'SegmentName'>;
export function parseSegmentName(input:unknown):Result<SegmentName,ParseError>{if(input==='segment-000000.jsonl')return failure('SegmentName','Segment numbering begins at one');return text(input,'SegmentName',/^segment-[0-9]{6}\.jsonl$/);}
export const segmentNameSchema=schemaFromParser(parseSegmentName);
export type OperationKind=Brand<`${string}.${string}`,'OperationKind'>;
export type CoreOperationKind='order.place'|'order.modify'|'order.cancel'|'position.close';
export function parseOperationKind(input:unknown):Result<OperationKind,ParseError>{
 if(typeof input!=='string'||input.length>512||!/^[a-z][a-z0-9_-]*(\.[a-z][a-z0-9_-]*)+$/.test(input))return failure('OperationKind','Expected namespaced dotted operation kind');
 return {kind:'ok',value:input as OperationKind};
}
export const operationKindSchema=schemaFromParser(parseOperationKind);
export function coreOperationKind(kind:OperationKind):Result<CoreOperationKind,{readonly kind:'UnknownKind';readonly operationKind:OperationKind}>{const name:string=kind;switch(name){case 'order.place':return {kind:'ok',value:'order.place'};case 'order.modify':return {kind:'ok',value:'order.modify'};case 'order.cancel':return {kind:'ok',value:'order.cancel'};case 'position.close':return {kind:'ok',value:'position.close'};}return {kind:'error',error:{kind:'UnknownKind',operationKind:kind}};}
export type Cursor=Brand<{readonly providerId:ProviderId;readonly channel:OperationKind;readonly value:string},'Cursor'>;
const cursorInputSchema=z.strictObject({providerId:providerIdSchema,channel:operationKindSchema,value:z.string().min(1)});
export function parseCursor(input:unknown):Result<Cursor,ParseError>{const parsed=cursorInputSchema.safeParse(input);return parsed.success?{kind:'ok',value:parsed.data as Cursor}:failure('Cursor',parsed.error.message);}
export const cursorSchema=schemaFromParser(parseCursor);
export async function sha256(text:string):Promise<Sha256>{const bytes=await crypto.subtle.digest('SHA-256',new TextEncoder().encode(text));return sha256Schema.parse(Array.from(new Uint8Array(bytes),byte=>byte.toString(16).padStart(2,'0')).join(''));}
export async function deriveConfigRevision(input:{readonly runtimeConfigDigest:Sha256;readonly accountConfigDigest:Sha256;readonly projectionVersion:ProjectionVersion}):Promise<ConfigRevision>{
 const digest=await sha256(JSON.stringify({accountConfigDigest:input.accountConfigDigest,projectionVersion:input.projectionVersion,runtimeConfigDigest:input.runtimeConfigDigest}));
 return configRevisionSchema.parse(digest);
}
