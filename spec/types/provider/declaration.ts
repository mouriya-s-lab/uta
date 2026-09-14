import { z } from 'zod';
import { providerIdSchema,projectionVersionSchema,sourceDigestSchema,operationKindSchema } from '../ids.ts';
import { durationSchema } from '../time.ts';
export const capabilityStatusSchema=z.discriminatedUnion('kind',[z.strictObject({kind:z.literal('supported')}),z.strictObject({kind:z.literal('unsupported'),reason:z.string().min(1)}),z.strictObject({kind:z.literal('conditional'),condition:z.string().min(1)})]);
export type CapabilityStatus=z.output<typeof capabilityStatusSchema>;
const none=z.strictObject({kind:z.literal('none')});
const cachedReplay=z.strictObject({kind:z.literal('cachedReplay'),keyField:z.string().min(1),scope:z.string().min(1),retention:durationSchema});
const uniqueKey=z.strictObject({kind:z.literal('uniqueKey'),keyField:z.string().min(1),scope:z.string().min(1),window:z.union([durationSchema,z.literal('untilTerminal'),z.literal('unverified')]),reuseAfterTerminal:z.union([z.boolean(),z.literal('unverified')]),duplicateResponse:z.enum(['rejectsWithCode','returnsExisting','unverified'])});
const nonNoneIdempotency=z.union([cachedReplay,uniqueKey]);
export const idempotencyContractSchema=z.union([none,cachedReplay,uniqueKey]);
export type IdempotencyContract=z.output<typeof idempotencyContractSchema>;
const coverage=z.enum(['openOnly','openAndHistory','unverified']);
const historyWindow=z.union([durationSchema,z.literal('unverified')]);
const clientRead=z.strictObject({kind:z.literal('byClientKey'),operation:z.string().min(1),coverage,historyWindow,ambiguity:z.enum(['unique','latestMatch'])});
const refRead=z.strictObject({kind:z.literal('byProviderRef'),coverage,historyWindow});
const nonNoneRead=z.union([clientRead,refRead]);
export const readByKeyContractSchema=z.union([none,clientRead,refRead]);
export type ReadByKeyContract=z.output<typeof readByKeyContractSchema>;
export const observationCursorContractSchema=z.union([none,z.strictObject({kind:z.literal('snapshotOnly')}),z.strictObject({kind:z.literal('resumable'),channel:operationKindSchema,encoding:z.string().min(1),replayWindow:durationSchema})]);
export type ObservationCursorContract=z.output<typeof observationCursorContractSchema>;
export const writeContractSchema=z.strictObject({idempotency:idempotencyContractSchema,readByKey:readByKeyContractSchema});
export type WriteContract=z.output<typeof writeContractSchema>;
export const recoverableWriteContractSchema=z.union([z.strictObject({idempotency:nonNoneIdempotency,readByKey:readByKeyContractSchema}),z.strictObject({idempotency:none,readByKey:nonNoneRead})]);
export type RecoverableWriteContract=z.output<typeof recoverableWriteContractSchema>;
export const capabilitySchema=z.union([
 z.strictObject({direction:z.literal('read'),status:capabilityStatusSchema,cursor:observationCursorContractSchema}),
 z.strictObject({direction:z.literal('write'),status:z.strictObject({kind:z.literal('supported')}),contract:recoverableWriteContractSchema}),
 z.strictObject({direction:z.literal('write'),status:z.strictObject({kind:z.literal('unsupported'),reason:z.string().min(1)}),contract:writeContractSchema}),
 z.strictObject({direction:z.literal('write'),status:z.strictObject({kind:z.literal('conditional'),condition:z.string().min(1)}),contract:writeContractSchema})
]);
export const capabilityTableSchema=z.record(operationKindSchema,capabilitySchema);
export type CapabilityTable=z.output<typeof capabilityTableSchema>;
export const providerDeclarationSchema=z.strictObject({providerId:providerIdSchema,projectionVersion:projectionVersionSchema,sourceDigest:sourceDigestSchema,venues:z.record(z.string().min(1),capabilityTableSchema),transportFamily:z.array(z.enum(['http','ws','custom'])).min(1),auth:z.array(z.strictObject({name:z.string().min(1),location:z.enum(['header','query','body','session'])}))});
export type ProviderDeclaration=z.output<typeof providerDeclarationSchema>;
export function defineDeclaration<const D extends ProviderDeclaration>(declaration:D):D{return declaration;}
