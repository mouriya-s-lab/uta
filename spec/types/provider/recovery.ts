import { z } from 'zod';
import type { Instant,AttemptNo,IntentId,IntentHash } from '../ids.ts';
import { instantSchema,intentHashSchema } from '../ids.ts';
import type { Duration } from '../time.ts';
import { durationSchema } from '../time.ts';
import { assertNever } from '../result.ts';
import type { Result,ParseError } from '../result.ts';
import type { WriteContract,ReadByKeyContract,RecoverableWriteContract } from './declaration.ts';
import type { ProviderTypes,ProviderKey,ProviderOrderRef,Receipt,Observation } from './indexed.ts';
import { receiptPayloadSchema,observationPayloadSchema,providerOrderRefSchema,providerKeySchema } from './indexed.ts';
export const verifiedReadByKeySchema=z.strictObject({kind:z.literal('byClientKey'),operation:z.string().min(1),coverage:z.literal('openAndHistory'),historyWindow:durationSchema,ambiguity:z.literal('unique')});
export type VerifiedReadByKey=z.output<typeof verifiedReadByKeySchema>;
export type RecoveryAttempt<P extends ProviderTypes>={readonly intentId:IntentId;readonly attemptNo:AttemptNo;readonly key:ProviderKey<P>;readonly providerRef?:ProviderOrderRef<P>|undefined;readonly startedAt:Instant;readonly unknownSince:Instant;readonly maxUnknownDuration:Duration;readonly payloadHash:IntentHash;readonly terminal:'knownNonterminal'|'terminal'|'unverified'};
export const absenceProofSchema=z.strictObject({kind:z.literal('keyedLookupMiss'),contract:verifiedReadByKeySchema,coverage:z.literal('openAndHistory'),window:durationSchema,attemptTime:instantSchema,checkedAt:instantSchema});
export type AbsenceProof=z.output<typeof absenceProofSchema>;
export type RecoveryResult<P extends ProviderTypes>=
 |{readonly kind:'found';readonly receipt:Extract<Receipt<P>,{kind:'accepted'}>&{readonly provenance:{readonly kind:'recovered';readonly via:'replay'|'keyedRead'|'providerRefRead'}};readonly observation:Observation<P>;readonly correlation:{readonly kind:'payloadMatch';readonly echoedKey:ProviderKey<P>;readonly payloadHash:IntentHash}|{readonly kind:'unverified'}}
 |{readonly kind:'confirmedAbsent';readonly proof:AbsenceProof;readonly validUntil:Instant}
 |{readonly kind:'ambiguous';readonly candidates:readonly {readonly providerRef:ProviderOrderRef<P>;readonly observation:Observation<P>}[]}
 |{readonly kind:'stillUnknown';readonly nextCheckAfter:Instant;readonly reason:string};
export type PlacementRecovery<P extends ProviderTypes>={readonly contract:RecoverableWriteContract;readonly recover:(attempt:RecoveryAttempt<P>,now:Instant)=>Promise<RecoveryResult<P>>};
export type RecoveryAction<P extends ProviderTypes>=
 |{readonly kind:'recordFound';readonly result:Extract<RecoveryResult<P>,{kind:'found'}>}
 |{readonly kind:'retrySameKey';readonly key:ProviderKey<P>;readonly proof:AbsenceProof}
 |{readonly kind:'replaySameKey';readonly key:ProviderKey<P>}
 |{readonly kind:'keyedLookup';readonly key:ProviderKey<P>}
 |{readonly kind:'providerRefLookup';readonly providerRef:ProviderOrderRef<P>}
 |{readonly kind:'scheduleRecheck';readonly at:Instant}
 |{readonly kind:'awaitingReview';readonly reason:string};
export function verifyReadByKey(contract:ReadByKeyContract):Result<VerifiedReadByKey,ParseError>{const parsed=verifiedReadByKeySchema.safeParse(contract);return parsed.success?{kind:'ok',value:parsed.data}:{kind:'error',error:{code:'InvalidInput',path:'ReadByKeyContract',message:'Absence needs verified unique full-history coverage'}};}
export function nextAction<P extends ProviderTypes>(declaration:WriteContract,result:RecoveryResult<P>,attempt:RecoveryAttempt<P>,now:Instant):RecoveryAction<P>{
 switch(result.kind){
 case 'found':
  if(declaration.readByKey.kind==='byClientKey'&&declaration.readByKey.ambiguity==='latestMatch'&&(result.correlation.kind!=='payloadMatch'||result.correlation.echoedKey!==attempt.key||result.correlation.payloadHash!==attempt.payloadHash))return {kind:'awaitingReview',reason:'AmbiguousRecovery'};
  return {kind:'recordFound',result};
 case 'ambiguous':return {kind:'awaitingReview',reason:'AmbiguousRecovery'};
 case 'stillUnknown':{
  const end=Math.min(Number.MAX_SAFE_INTEGER,attempt.unknownSince+attempt.maxUnknownDuration);
  if(now>=end)return {kind:'awaitingReview',reason:'UnknownDurationExceeded'};
  return {kind:'scheduleRecheck',at:instantSchema.parse(Math.min(end,Math.max(now+1,result.nextCheckAfter)))};
 }
 case 'confirmedAbsent':{
  const proof=result.proof,verified=verifyReadByKey(declaration.readByKey);
  if(verified.kind==='error'||verified.value.operation!==proof.contract.operation||verified.value.historyWindow!==proof.window||proof.window!==proof.contract.historyWindow)return {kind:'awaitingReview',reason:'AbsenceContractMismatch'};
  if(now>=result.validUntil||proof.attemptTime!==attempt.startedAt||proof.checkedAt<attempt.startedAt||now-attempt.startedAt>=proof.window||proof.checkedAt>now)return {kind:'awaitingReview',reason:'AbsenceProofExpired'};
  const contract=declaration.idempotency;
  switch(contract.kind){
   case 'none':return {kind:'awaitingReview',reason:'UnsafeRetry'};
   case 'cachedReplay':return now-attempt.startedAt<contract.retention?{kind:'retrySameKey',key:attempt.key,proof}:{kind:'awaitingReview',reason:'ReplayWindowExpired'};
   case 'uniqueKey':{const inside=contract.window==='untilTerminal'?attempt.terminal==='knownNonterminal':contract.window!=='unverified'&&now-attempt.startedAt<contract.window;return contract.duplicateResponse!=='unverified'&&inside?{kind:'retrySameKey',key:attempt.key,proof}:{kind:'awaitingReview',reason:'UnsafeRetry'};}
  }
  return assertNever(contract);
 }
 }
 return assertNever(result);
}
export function recoveryProbe<P extends ProviderTypes>(declaration:WriteContract,attempt:RecoveryAttempt<P>,now:Instant):RecoveryAction<P>{
 if(now-attempt.unknownSince>=attempt.maxUnknownDuration)return {kind:'awaitingReview',reason:'UnknownDurationExceeded'};
 switch(declaration.readByKey.kind){case 'byClientKey':return {kind:'keyedLookup',key:attempt.key};case 'byProviderRef':if(attempt.providerRef)return {kind:'providerRefLookup',providerRef:attempt.providerRef};break;case 'none':break;}
 const contract=declaration.idempotency;
 switch(contract.kind){case 'cachedReplay':return now>=attempt.startedAt&&now-attempt.startedAt<contract.retention?{kind:'replaySameKey',key:attempt.key}:{kind:'awaitingReview',reason:'ReplayWindowExpired'};case 'uniqueKey':return {kind:'awaitingReview',reason:'NoKeyedLookup'};case 'none':return {kind:'awaitingReview',reason:'NoPlacementRecovery'};}
 return assertNever(contract);
}
const recoveredReceipt=receiptPayloadSchema.options[0].extend({provenance:z.strictObject({kind:z.literal('recovered'),via:z.enum(['replay','keyedRead','providerRefRead'])})});
// Runtime return validation uses an existential key representation; concrete P is restored by registry codecs.
export const recoveryResultSchema=z.discriminatedUnion('kind',[
 z.strictObject({kind:z.literal('found'),receipt:recoveredReceipt,observation:observationPayloadSchema,correlation:z.union([z.strictObject({kind:z.literal('unverified')}),z.strictObject({kind:z.literal('payloadMatch'),echoedKey:providerKeySchema,payloadHash:intentHashSchema})])}),
 z.strictObject({kind:z.literal('confirmedAbsent'),proof:absenceProofSchema,validUntil:instantSchema}),
 z.strictObject({kind:z.literal('ambiguous'),candidates:z.array(z.strictObject({providerRef:providerOrderRefSchema,observation:observationPayloadSchema}))}),
 z.strictObject({kind:z.literal('stillUnknown'),nextCheckAfter:instantSchema,reason:z.string().min(1)})
]);
