import { z } from 'zod';
import { accountIdSchema,requestIdSchema,intentIdSchema,idempotencyKeySchema,entryIdSchema,entryPositionSchema,instantSchema,providerIdSchema,projectionVersionSchema,decisionIdSchema,scopeHashSchema,intentHashSchema,configRevisionSchema } from './ids.ts';
import { freshnessSchema,durationSchema } from './time.ts';
import { operationEnvelopeSchema } from './provider/indexed.ts';
import { reasonTreeSchema } from './ledger/entries.ts';
export const decisionActionSchema=z.enum(['approve','reject','withdraw']);
export type DecisionAction=z.output<typeof decisionActionSchema>;
export const decisionRequestSchema=z.strictObject({decisionId:decisionIdSchema,action:decisionActionSchema,scopeHash:scopeHashSchema});
export type DecisionRequest=z.output<typeof decisionRequestSchema>;
export const decisionResponseSchema=z.strictObject({kind:z.literal('recorded'),entryId:entryIdSchema,position:entryPositionSchema,authority:z.enum(['binding','recommendation'])});
export type DecisionResponse=z.output<typeof decisionResponseSchema>;
// Laziness defers the runtime schema cycle and keeps one canonical ReasonTree parser.
export const workRequestedSchema=z.lazy(()=>{
 const common={requestId:requestIdSchema,accountId:accountIdSchema,causal:z.strictObject({entryId:entryIdSchema,position:entryPositionSchema,why:reasonTreeSchema}),authority:z.enum(['binding','recommendation']),scopeHash:scopeHashSchema,intentHash:intentHashSchema,freshness:freshnessSchema,providerId:providerIdSchema,projectionVersion:projectionVersionSchema,operation:operationEnvelopeSchema,what:z.string().min(1),expiresAt:instantSchema};
 const review={...common,intentId:intentIdSchema,idempotencyKey:idempotencyKeySchema,admissibleDecisions:z.tuple([decisionActionSchema],decisionActionSchema)};
 return z.discriminatedUnion('kind',[
  z.strictObject({...review,kind:z.literal('review.unknownOutcome')}),z.strictObject({...review,kind:z.literal('review.ambiguousRecovery')}),
  z.strictObject({...review,kind:z.literal('review.ruleRejection'),consumer:z.string().min(1),ruleId:z.string().min(1),configRevision:configRevisionSchema,reasonTree:reasonTreeSchema}),
  z.strictObject({...review,kind:z.literal('reconcile.discrepancy')}),z.strictObject({...review,kind:z.literal('intent.awaitingAuthorization')}),
  z.strictObject({...common,kind:z.literal('watch.triggered'),intentId:intentIdSchema.optional(),idempotencyKey:idempotencyKeySchema.optional(),admissibleDecisions:z.array(decisionActionSchema)}),
  z.strictObject({...common,kind:z.literal('news.received'),intentId:intentIdSchema.optional(),idempotencyKey:idempotencyKeySchema.optional(),admissibleDecisions:z.array(decisionActionSchema)})
 ]).superRefine((work,context)=>{if(work.providerId!==work.operation.providerId||work.projectionVersion!==work.operation.projectionVersion)context.addIssue({code:'custom',message:'Work request projection identity mismatch'});});
});
export type WorkRequested=z.output<typeof workRequestedSchema>;
export const utaDeskSettingsSchema=z.strictObject({workspaceId:z.string().min(1),deskIssueId:z.string().regex(/^[A-Za-z0-9_-]{1,120}$/),pollIntervalMs:durationSchema});
export type UtaDeskSettings=z.output<typeof utaDeskSettingsSchema>;
export const deskBridgeLinkSchema=z.strictObject({requestId:requestIdSchema,workspaceId:z.string().min(1),issueId:z.string().regex(/^[A-Za-z0-9_-]{1,120}$/),commentId:z.string().regex(/^[A-Za-z0-9_-]{1,120}$/).optional(),sourcePosition:entryPositionSchema});
export type DeskBridgeLink=z.output<typeof deskBridgeLinkSchema>;
export const deskBridgeLinkStoreSchema=z.strictObject({version:z.literal(1),accountId:accountIdSchema,links:z.array(deskBridgeLinkSchema)}).superRefine((store,context)=>{if(new Set(store.links.map(link=>link.requestId)).size!==store.links.length)context.addIssue({code:'custom',message:'Duplicate request link'});});
export type DeskBridgeLinkStore=z.output<typeof deskBridgeLinkStoreSchema>;
