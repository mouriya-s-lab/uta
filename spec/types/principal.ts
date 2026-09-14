import { z } from 'zod';
import { entryIdSchema,entryPositionSchema,cursorSchema } from './ids.ts';
import { providerEventIdSchema } from './provider/indexed.ts';
const opaque=z.string().min(1);
export const principalSchema=z.discriminatedUnion('kind',[
 z.strictObject({kind:z.literal('human'),sessionId:opaque}),z.strictObject({kind:z.literal('agent'),workspaceId:opaque,resumeId:opaque,runId:opaque}),
 z.strictObject({kind:z.literal('connector'),connectorId:opaque,externalUserId:opaque.optional()}),z.strictObject({kind:z.literal('schedule'),workspaceId:opaque,issueId:opaque}),
 z.strictObject({kind:z.literal('policy'),policyId:opaque}),z.strictObject({kind:z.literal('system')})
]);
export type Principal=z.output<typeof principalSchema>;
export type PrincipalKind=Principal['kind'];
const eventOrigin={sourceEntryId:entryIdSchema,sourcePosition:entryPositionSchema,cursor:cursorSchema.optional(),eventId:providerEventIdSchema.optional()};
export const originSchema=z.discriminatedUnion('kind',[
 z.strictObject({kind:z.literal('issue'),issueId:opaque,resumeId:opaque,runId:opaque}),z.strictObject({kind:z.literal('direct')}),
 z.strictObject({kind:z.literal('connector'),connectorId:opaque}),z.strictObject({kind:z.literal('schedule'),issueId:opaque}),z.strictObject({kind:z.literal('policy'),policyId:opaque}),
 z.strictObject({kind:z.literal('watch'),...eventOrigin}),z.strictObject({kind:z.literal('news'),...eventOrigin})
]);
export type Origin=z.output<typeof originSchema>;
