import { z } from 'zod';
import { accountIdSchema,intentIdSchema,proposalIdSchema,decisionIdSchema,entryIdSchema,entryPositionSchema,instantSchema,idempotencyKeySchema,attemptNoSchema,configRevisionSchema,operationKindSchema,intentHashSchema,scopeHashSchema,sourceSchema } from '../ids.ts';
import { principalSchema,originSchema } from '../principal.ts';
import { operationEnvelopeSchema,keyEnvelopeSchema,receiptEnvelopeSchema,observationEnvelopeSchema,providerOrderRefSchema,serializableSchema } from '../provider/indexed.ts';
import { absenceProofSchema,recoveryResultSchema } from '../provider/recovery.ts';
import { workRequestedSchema,decisionActionSchema } from '../issues.ts';
import type { OperationKind } from '../ids.ts';
import type { Result,ParseError } from '../result.ts';
export type ReasonTree={readonly kind:'Because';readonly code:string;readonly message:string}|{readonly kind:'UnknownKind';readonly operationKind:OperationKind}|{readonly kind:'RuleRejected';readonly ruleId:string;readonly children:readonly [ReasonTree,...ReasonTree[]]}|{readonly kind:'Pending';readonly reason:string};
export const reasonTreeSchema:z.ZodType<ReasonTree>=z.lazy(()=>z.union([z.strictObject({kind:z.literal('Because'),code:z.string().min(1),message:z.string().min(1)}),z.strictObject({kind:z.literal('UnknownKind'),operationKind:operationKindSchema}),z.strictObject({kind:z.literal('RuleRejected'),ruleId:z.string().min(1),children:z.tuple([reasonTreeSchema],reasonTreeSchema)}),z.strictObject({kind:z.literal('Pending'),reason:z.string().min(1)})]));
export const decisionScopeSchema=z.discriminatedUnion('kind',[z.strictObject({kind:z.literal('proposal'),proposalId:proposalIdSchema}),z.strictObject({kind:z.literal('intents'),intentIds:z.tuple([intentIdSchema],intentIdSchema)})]);
export type DecisionScope=z.output<typeof decisionScopeSchema>;
export const policyResultSchema=z.discriminatedUnion('kind',[z.strictObject({kind:z.literal('binding')}),z.strictObject({kind:z.literal('recommendation')}),z.strictObject({kind:z.literal('rejected'),code:z.enum(['IntentExpired','DecisionConflict']),reason:reasonTreeSchema})]);
export type PolicyResult=z.output<typeof policyResultSchema>;
const recoveryRecorded=z.discriminatedUnion('kind',[
 z.strictObject({kind:z.literal('found'),receiptEntryId:entryIdSchema,observationEntryId:entryIdSchema}),
 recoveryResultSchema.options[1],recoveryResultSchema.options[2],recoveryResultSchema.options[3],
 z.strictObject({kind:z.literal('manual'),principal:principalSchema,resolution:z.enum(['awaitingReview','abandon','continue']),reason:reasonTreeSchema})
]);
export const ledgerPayloadSchemas={
 'intent.proposed':z.strictObject({intentId:intentIdSchema,proposalId:proposalIdSchema.optional(),intentHash:intentHashSchema,principal:principalSchema,origin:originSchema,scope:z.strictObject({accountId:accountIdSchema,operationKind:operationKindSchema,operation:operationEnvelopeSchema}),expiresAt:instantSchema,configRevision:configRevisionSchema}),
 'authorization.decided':z.strictObject({decisionId:decisionIdSchema,scope:decisionScopeSchema,scopeHash:scopeHashSchema,action:decisionActionSchema,principal:principalSchema,policyResult:policyResultSchema}),
 'intent.expired':z.strictObject({intentId:intentIdSchema}),
 'attempt.started':z.strictObject({intentId:intentIdSchema,attemptNo:attemptNoSchema,key:keyEnvelopeSchema,configRevision:configRevisionSchema,deadline:instantSchema}),
 'attempt.abandoned':z.strictObject({intentId:intentIdSchema,attemptNo:attemptNoSchema,reason:z.discriminatedUnion('kind',[z.strictObject({kind:z.literal('notSent'),cause:z.string().min(1)}),z.strictObject({kind:z.literal('confirmedAbsent'),proof:absenceProofSchema})])}),
 'receipt.recorded':z.strictObject({intentId:intentIdSchema,attemptNo:attemptNoSchema,receipt:receiptEnvelopeSchema}),
 'recovery.resolved':z.strictObject({intentId:intentIdSchema,attemptNo:attemptNoSchema,result:recoveryRecorded}),
 'observation.recorded':z.strictObject({observation:observationEnvelopeSchema}),
 'reversal.appended':z.strictObject({target:entryIdSchema,consumer:z.string().min(1),reasonTree:reasonTreeSchema}),
 'work.requested':workRequestedSchema
};
export type LedgerPayloads={[K in keyof typeof ledgerPayloadSchemas]:z.output<(typeof ledgerPayloadSchemas)[K]>};
export const entryEnvelopeSchema=z.strictObject({schemaVersion:z.literal(2),accountId:accountIdSchema,entryId:entryIdSchema,position:entryPositionSchema,occurredAt:instantSchema,recordedAt:instantSchema,source:sourceSchema,idempotencyKey:idempotencyKeySchema,why:reasonTreeSchema,causedBy:entryIdSchema.optional(),correlation:z.strictObject({intentId:intentIdSchema.optional(),attemptNo:attemptNoSchema.optional(),providerRef:providerOrderRefSchema.optional()})});
export type EntryEnvelope=z.output<typeof entryEnvelopeSchema>;
export type EntryOf<K extends keyof LedgerPayloads>=EntryEnvelope&{readonly kind:K;readonly payload:LedgerPayloads[K]};
export type LedgerEntry={[K in keyof LedgerPayloads]:EntryOf<K>}[keyof LedgerPayloads];
export const ledgerEntrySchema:z.ZodType<LedgerEntry>=z.discriminatedUnion('kind',[
 entryEnvelopeSchema.extend({kind:z.literal('intent.proposed'),payload:ledgerPayloadSchemas['intent.proposed']}),
 entryEnvelopeSchema.extend({kind:z.literal('authorization.decided'),payload:ledgerPayloadSchemas['authorization.decided']}),
 entryEnvelopeSchema.extend({kind:z.literal('intent.expired'),payload:ledgerPayloadSchemas['intent.expired']}),
 entryEnvelopeSchema.extend({kind:z.literal('attempt.started'),payload:ledgerPayloadSchemas['attempt.started']}),
 entryEnvelopeSchema.extend({kind:z.literal('attempt.abandoned'),payload:ledgerPayloadSchemas['attempt.abandoned']}),
 entryEnvelopeSchema.extend({kind:z.literal('receipt.recorded'),payload:ledgerPayloadSchemas['receipt.recorded']}),
 entryEnvelopeSchema.extend({kind:z.literal('recovery.resolved'),payload:ledgerPayloadSchemas['recovery.resolved']}),
 entryEnvelopeSchema.extend({kind:z.literal('observation.recorded'),payload:ledgerPayloadSchemas['observation.recorded']}),
 entryEnvelopeSchema.extend({kind:z.literal('reversal.appended'),payload:ledgerPayloadSchemas['reversal.appended']}),
 entryEnvelopeSchema.extend({kind:z.literal('work.requested'),payload:ledgerPayloadSchemas['work.requested']})
]).superRefine((entry,context)=>{
 const payload=entry.payload;
 if('intentId' in payload&&payload.intentId!==undefined&&entry.correlation.intentId!==payload.intentId)context.addIssue({code:'custom',path:['correlation','intentId'],message:'Intent correlation mismatch'});
 if('attemptNo' in payload&&entry.correlation.attemptNo!==payload.attemptNo)context.addIssue({code:'custom',path:['correlation','attemptNo'],message:'Attempt correlation mismatch'});
 if(entry.kind==='intent.proposed'&&(entry.accountId!==entry.payload.scope.accountId||entry.payload.scope.operationKind!==entry.payload.scope.operation.kind))context.addIssue({code:'custom',message:'Intent scope mismatch'});
 if(entry.kind==='work.requested'&&entry.accountId!==entry.payload.accountId)context.addIssue({code:'custom',message:'Work request account mismatch'});
 const ref=entry.kind==='receipt.recorded'&&entry.payload.receipt.payload.kind==='accepted'?entry.payload.receipt.payload.providerRef:entry.kind==='observation.recorded'?entry.payload.observation.payload.providerRef:undefined;
 if(ref!==undefined&&entry.correlation.providerRef!==ref)context.addIssue({code:'custom',path:['correlation','providerRef'],message:'Provider reference correlation mismatch'});
});
const knownKinds=new Set<string>(Object.keys(ledgerPayloadSchemas));
export const unknownLedgerEntrySchema=entryEnvelopeSchema.extend({kind:z.string().min(1).refine(kind=>!knownKinds.has(kind),'Known kinds require their specific parser'),payload:serializableSchema,consumption:z.literal('unconsumed')});
export type UnknownLedgerEntry=z.output<typeof unknownLedgerEntrySchema>;
export function parseLedgerEntry(input:unknown):Result<LedgerEntry|UnknownLedgerEntry,{readonly kind:'Corrupt';readonly reason:ParseError}>{
 const tag=z.object({kind:z.string()}).safeParse(input);
 if(tag.success&&knownKinds.has(tag.data.kind)){
  const parsed=ledgerEntrySchema.safeParse(input);
  return parsed.success?{kind:'ok',value:parsed.data}:{kind:'error',error:{kind:'Corrupt',reason:{code:'InvalidInput',path:'LedgerEntry',message:parsed.error.message}}};
 }
 const parsed=unknownLedgerEntrySchema.safeParse(input);
 return parsed.success?{kind:'ok',value:parsed.data}:{kind:'error',error:{kind:'Corrupt',reason:{code:'InvalidInput',path:'LedgerEntry',message:parsed.error.message}}};
}
