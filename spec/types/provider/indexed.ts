import { z } from 'zod';
import type { Brand,ProviderId,ProjectionVersion,OperationKind,AsOf,Cursor,RequestId,ProviderErrorCode } from '../ids.ts';
import { providerIdSchema,projectionVersionSchema,operationKindSchema,asOfSchema,cursorSchema,requestIdSchema,providerErrorCodeSchema,schemaFromParser } from '../ids.ts';
import type { DecimalString } from '../money.ts';
import type { Result,ParseError } from '../result.ts';
import type { PlacementRecovery } from './recovery.ts';
export type Serializable=null|boolean|number|string|readonly Serializable[]|{readonly [key:string]:Serializable};
export const serializableSchema:z.ZodType<Serializable>=z.lazy(()=>z.preprocess(input=>{if(typeof input==='object'&&input!==null&&!Array.isArray(input)&&Object.getPrototypeOf(input)!==Object.prototype&&Object.getPrototypeOf(input)!==null)return undefined;return input;},z.union([z.null(),z.boolean(),z.number().finite(),z.string(),z.array(serializableSchema),z.record(z.string(),serializableSchema)])));
export type ProviderTypes={readonly identity:ProviderId;readonly projectionVersion:ProjectionVersion;readonly operation:Serializable;readonly receipt:Serializable;readonly observation:Serializable};
type ProviderIndex<P extends ProviderTypes>={readonly providerId:P['identity'];readonly projectionVersion:P['projectionVersion']};
export type ProviderKey<P extends ProviderTypes>=Brand<string,{readonly role:'key';readonly provider:ProviderIndex<P>}>;
export type ProviderOrderRef<P extends ProviderTypes>=Brand<string,{readonly role:'orderRef';readonly provider:ProviderIndex<P>}>;
export type ProviderEventId<P extends ProviderTypes>=Brand<string,{readonly role:'eventId';readonly provider:ProviderIndex<P>}>;
export function parseProviderKey<P extends ProviderTypes>(input:unknown,provider:P):Result<ProviderKey<P>,ParseError>{
 if(typeof input!=='string'||!input.trim()||!provider.identity||!provider.projectionVersion)return {kind:'error',error:{code:'InvalidInput',path:'ProviderKey',message:'Invalid scoped key'}};
 // PARSER BOUNDARY: registry-resolved provider/version + checked opaque representation.
 return {kind:'ok',value:input as ProviderKey<P>};
}
export function parseProviderOrderRef<P extends ProviderTypes>(input:unknown,provider:P):Result<ProviderOrderRef<P>,ParseError>{
 if(typeof input!=='string'||!input.trim()||!provider.identity||!provider.projectionVersion)return {kind:'error',error:{code:'InvalidInput',path:'ProviderOrderRef',message:'Invalid scoped reference'}};
 // PARSER BOUNDARY: registry-resolved provider/version + checked opaque representation.
 return {kind:'ok',value:input as ProviderOrderRef<P>};
}
export function parseProviderEventId<P extends ProviderTypes>(input:unknown,provider:P):Result<ProviderEventId<P>,ParseError>{
 if(typeof input!=='string'||!input.trim()||!provider.identity||!provider.projectionVersion)return {kind:'error',error:{code:'InvalidInput',path:'ProviderEventId',message:'Invalid scoped event id'}};
 // PARSER BOUNDARY: registry-resolved provider/version + checked opaque representation.
 return {kind:'ok',value:input as ProviderEventId<P>};
}
// Existential wire refs are parsed inside an envelope, then re-associated by its provider/version.
export const providerOrderRefSchema=schemaFromParser((input:unknown):Result<ProviderOrderRef<ProviderTypes>,ParseError>=>{
 if(typeof input!=='string'||!input.trim())return {kind:'error',error:{code:'InvalidInput',path:'ProviderOrderRef',message:'Expected opaque reference'}};
 // PARSER BOUNDARY: this is existential, not a claim of association with a concrete P.
 return {kind:'ok',value:input as ProviderOrderRef<ProviderTypes>};
});
export const providerEventIdSchema=schemaFromParser((input:unknown):Result<ProviderEventId<ProviderTypes>,ParseError>=>{
 if(typeof input!=='string'||!input.trim())return {kind:'error',error:{code:'InvalidInput',path:'ProviderEventId',message:'Expected opaque event identity'}};
 // PARSER BOUNDARY: existential event identity; registry performs concrete association.
 return {kind:'ok',value:input as ProviderEventId<ProviderTypes>};
});
export const receiptProvenanceSchema=z.discriminatedUnion('kind',[z.strictObject({kind:z.literal('original')}),z.strictObject({kind:z.literal('recovered'),via:z.enum(['replay','keyedRead','providerRefRead'])})]);
export type ReceiptProvenance=z.output<typeof receiptProvenanceSchema>;
export const unknownCauseSchema=z.enum(['timeout','disconnect','noResponse','processRestart','parseFailure']);
export type UnknownCause=z.output<typeof unknownCauseSchema>;
export type Operation<P extends ProviderTypes>={readonly kind:OperationKind;readonly key:ProviderKey<P>;readonly payload:P['operation']};
export type Receipt<P extends ProviderTypes>=
 |{readonly kind:'accepted';readonly providerRef:ProviderOrderRef<P>;readonly provenance:ReceiptProvenance;readonly raw:P['receipt']}
 |{readonly kind:'rejected';readonly code:ProviderErrorCode;readonly requestId:RequestId;readonly raw:P['receipt']}
 |{readonly kind:'unknown';readonly cause:UnknownCause};
export type Observation<P extends ProviderTypes>={readonly asOf:AsOf;readonly source:ProviderId;readonly providerRef?:ProviderOrderRef<P>|undefined;readonly eventId?:ProviderEventId<P>|undefined;readonly cursor?:Cursor|undefined;readonly state:P['observation'];readonly raw:Serializable};
export const receiptPayloadSchema=z.discriminatedUnion('kind',[
 z.strictObject({kind:z.literal('accepted'),providerRef:providerOrderRefSchema,provenance:receiptProvenanceSchema,raw:serializableSchema}),
 z.strictObject({kind:z.literal('rejected'),code:providerErrorCodeSchema,requestId:requestIdSchema,raw:serializableSchema}),
 z.strictObject({kind:z.literal('unknown'),cause:unknownCauseSchema})
]);
export const observationPayloadSchema=z.strictObject({asOf:asOfSchema,source:providerIdSchema,providerRef:providerOrderRefSchema.optional(),eventId:providerEventIdSchema.optional(),cursor:cursorSchema.optional(),state:serializableSchema,raw:serializableSchema});
const envelope={providerId:providerIdSchema,projectionVersion:projectionVersionSchema,kind:operationKindSchema};
export const operationEnvelopeSchema=z.strictObject({...envelope,role:z.literal('operation'),payload:serializableSchema});
export const keyEnvelopeSchema=z.strictObject({...envelope,role:z.literal('key'),payload:z.string().min(1)});
export const receiptEnvelopeSchema=z.strictObject({...envelope,role:z.literal('receipt'),payload:receiptPayloadSchema});
export const observationEnvelopeSchema=z.strictObject({...envelope,role:z.literal('observation'),payload:observationPayloadSchema}).superRefine((value,context)=>{if(value.payload.source!==value.providerId)context.addIssue({code:'custom',message:'Observation source mismatches envelope provider'});if(value.payload.cursor&&value.payload.cursor.providerId!==value.providerId)context.addIssue({code:'custom',message:'Observation cursor mismatches provider'});});
export const configEnvelopeSchema=z.strictObject({...envelope,role:z.literal('config'),payload:serializableSchema});
export const providerEnvelopeSchema=z.union([operationEnvelopeSchema,keyEnvelopeSchema,receiptEnvelopeSchema,observationEnvelopeSchema,configEnvelopeSchema]);
export type OperationEnvelope=z.output<typeof operationEnvelopeSchema>;
export type KeyEnvelope=z.output<typeof keyEnvelopeSchema>;
export type ReceiptEnvelope=z.output<typeof receiptEnvelopeSchema>;
export type ObservationEnvelope=z.output<typeof observationEnvelopeSchema>;
export type ConfigEnvelope=z.output<typeof configEnvelopeSchema>;
export type ProviderEnvelope=z.output<typeof providerEnvelopeSchema>;
export type OrderState={readonly kind:'open'|'filled'|'partiallyFilled'|'cancelled';readonly filledQty:DecimalString};
export interface IndexedProjection<P extends ProviderTypes>{
 readonly providerId:P['identity'];readonly projectionVersion:P['projectionVersion'];
 decodeOperation(input:OperationEnvelope):Result<Operation<P>,ParseError>;
 decodeReceipt(input:ReceiptEnvelope):Result<Receipt<P>,ParseError>;
 decodeObservation(input:ObservationEnvelope):Result<Observation<P>,ParseError>;
 recoveryFor(kind:OperationKind):Result<PlacementRecovery<P>,ParseError>;
}
export interface ProjectionRegistry{
 reassociate<T>(envelope:ProviderEnvelope,consume:<P extends ProviderTypes>(projection:IndexedProjection<P>)=>Result<T,ParseError>):Result<T,ParseError>;
}
// PARSER BOUNDARY: existential representation; no concrete provider/version association asserted.
export const providerKeySchema=z.string().min(1).transform(input=>input as ProviderKey<ProviderTypes>);
