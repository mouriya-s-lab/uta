import { z } from 'zod';
import type { ProviderDeclaration } from './declaration.ts';
import { providerDeclarationSchema,writeContractSchema,recoverableWriteContractSchema } from './declaration.ts';
import type { Serializable,ProviderTypes,ProviderEventId,Operation,Receipt,Observation } from './indexed.ts';
import { serializableSchema,providerKeySchema,receiptPayloadSchema,observationPayloadSchema,providerEventIdSchema } from './indexed.ts';
import type { PlacementRecovery } from './recovery.ts';
import { recoveryResultSchema } from './recovery.ts';
import type { Result,ParseError } from '../result.ts';
import { parseErrorSchema,resultSchema } from '../result.ts';
import { operationKindSchema,requestIdSchema,instantSchema,asOfSchema,cursorSchema } from '../ids.ts';
import type { OperationKind,RequestId,Instant,AsOf,Cursor } from '../ids.ts';
import { durationSchema } from '../time.ts';
import type { Duration } from '../time.ts';
export const BROKER_PACK_API_VERSION=2;
export type CredentialInput=readonly {readonly name:string;readonly value:string}[];
export type TransportCause={readonly kind:'network';readonly code:string;readonly message:string}|{readonly kind:'aborted'}|{readonly kind:'deadline'};
const transportCauseSchema=z.discriminatedUnion('kind',[z.strictObject({kind:z.literal('network'),code:z.string().min(1),message:z.string().min(1)}),z.strictObject({kind:z.literal('aborted')}),z.strictObject({kind:z.literal('deadline')})]);
export type TransportRequest={readonly kind:OperationKind;readonly payload:Serializable;readonly deadline:Duration;readonly control:{readonly signal?:AbortSignal|undefined}};
export const transportRequestSchema=z.strictObject({kind:operationKindSchema,payload:serializableSchema,deadline:durationSchema,control:z.strictObject({signal:z.instanceof(AbortSignal).optional()})});
export const transportResultSchema=z.discriminatedUnion('kind',[
 z.strictObject({kind:z.literal('responded'),status:z.number().int().min(100).max(599),requestId:requestIdSchema.optional(),rawBody:z.string(),providerTime:instantSchema.optional(),payload:serializableSchema}),
 z.strictObject({kind:z.literal('notSent'),cause:z.union([parseErrorSchema,transportCauseSchema])}),
 z.strictObject({kind:z.literal('unknown'),cause:z.enum(['timeout','disconnect','noResponse','parseFailure']),sendEvidence:z.strictObject({kind:z.literal('sent'),at:instantSchema,requestId:requestIdSchema.optional()})})
]);
export type TransportResult=z.output<typeof transportResultSchema>;
export const streamChannelSchema=z.strictObject({name:operationKindSchema,cursor:z.enum(['resumable','snapshotOnly']),endReasons:z.array(z.enum(['closedByProvider','deadline','drain','authRevoked'])).min(1)});
export type StreamChannel=z.output<typeof streamChannelSchema>;
export const streamManifestSchema=z.strictObject({version:z.literal(1),channels:z.array(streamChannelSchema).min(1)});
export type StreamManifest=z.output<typeof streamManifestSchema>;
export type StreamItem<P extends ProviderTypes>=
 |{readonly kind:'event';readonly channel:OperationKind;readonly eventId?:ProviderEventId<P>|undefined;readonly asOf:AsOf;readonly providerTime?:Instant|undefined;readonly cursor?:Cursor|undefined;readonly payload:Serializable}
 |{readonly kind:'gap';readonly channel:OperationKind;readonly reason:'cursorExpired'|'resubscribed'|'providerReset';readonly lastCursor?:Cursor|undefined}
 |{readonly kind:'ended';readonly channel:OperationKind;readonly reason:'closedByProvider'|'deadline'|'drain'|'authRevoked'};
export const streamItemSchema=z.discriminatedUnion('kind',[
 z.strictObject({kind:z.literal('event'),channel:operationKindSchema,eventId:providerEventIdSchema.optional(),asOf:asOfSchema,providerTime:instantSchema.optional(),cursor:cursorSchema.optional(),payload:serializableSchema}),
 z.strictObject({kind:z.literal('gap'),channel:operationKindSchema,reason:z.enum(['cursorExpired','resubscribed','providerReset']),lastCursor:cursorSchema.optional()}),
 z.strictObject({kind:z.literal('ended'),channel:operationKindSchema,reason:z.enum(['closedByProvider','deadline','drain','authRevoked'])})
]);
export interface HTTPExecutor{readonly kind:'http';execute(request:TransportRequest,credentials:CredentialInput):Promise<TransportResult>;}
export interface WSExecutor{readonly kind:'ws';open(channel:StreamChannel,request:TransportRequest,credentials:CredentialInput):Promise<{readonly events:AsyncIterable<StreamItem<ProviderTypes>>;close():Promise<void>}>;}
export interface CustomPlugin{readonly kind:'custom';readonly protocol:string;execute(request:TransportRequest,credentials:CredentialInput):Promise<TransportResult>;close():Promise<void>;}
export type TransportPlugin=HTTPExecutor|WSExecutor|CustomPlugin;
export type Translation<P extends ProviderTypes>={readonly encodeOperation:(operation:Operation<P>)=>Result<Serializable,ParseError>;readonly decodeOperation:(input:Serializable)=>Result<Operation<P>,ParseError>;readonly decodeReceipt:(input:Serializable)=>Result<Receipt<P>,ParseError>;readonly decodeObservation:(input:Serializable)=>Result<Observation<P>,ParseError>};
export type ProviderFor<D extends ProviderDeclaration>={readonly identity:D['providerId'];readonly projectionVersion:D['projectionVersion'];readonly operation:Serializable;readonly receipt:Serializable;readonly observation:Serializable};
type SupportedKinds<D extends ProviderDeclaration,V extends keyof D['venues']>={[K in keyof D['venues'][V]]:Extract<D['venues'][V][K],{direction:'write';status:{kind:'supported'}}> extends never?never:K}[keyof D['venues'][V]];
type ContractAt<D extends ProviderDeclaration,V extends keyof D['venues'],K extends keyof D['venues'][V]>=Extract<D['venues'][V][K],{direction:'write';status:{kind:'supported'}}> extends {contract:infer C}?C:never;
export type PackModule<D extends ProviderDeclaration>={
 readonly BROKER_PACK_API_VERSION:2;readonly declaration:D;readonly translation:Translation<ProviderFor<D>>;readonly transports:readonly TransportPlugin[];readonly streamManifest?:StreamManifest|undefined;
 readonly recovery:{readonly [V in keyof D['venues'] as SupportedKinds<D,V> extends never?never:V]:{readonly [K in SupportedKinds<D,V>]:PlacementRecovery<ProviderFor<D>>&{readonly contract:ContractAt<D,V,K>}}};
 readonly bindings:{readonly [V in keyof D['venues'] as SupportedKinds<D,V> extends never?never:V]:{readonly [K in SupportedKinds<D,V>]:TransportPlugin}};
};
export type LoaderValidationResult<D extends ProviderDeclaration=ProviderDeclaration>={readonly kind:'loaded';readonly module:PackModule<D>}|{readonly kind:'invalid';readonly code:'ApiVersion'|'DeclarationInvalid'|'MissingHandler'|'NonSerializable'|'ProjectionIdentity';readonly path:string};
export interface PackLoader{load(input:unknown,expected?:{readonly providerId:ProviderDeclaration['providerId'];readonly projectionVersion:ProviderDeclaration['projectionVersion']}):LoaderValidationResult;}
const callable=z.custom<(...args:unknown[])=>unknown>(value=>typeof value==='function');
const rawPluginSchema=z.discriminatedUnion('kind',[z.object({kind:z.literal('http'),execute:callable}),z.object({kind:z.literal('custom'),protocol:z.string().min(1),execute:callable,close:callable}),z.object({kind:z.literal('ws'),open:callable})]);
const asyncValues=z.custom<AsyncIterable<unknown>>(value=>typeof value==='object'&&value!==null&&Symbol.asyncIterator in value&&typeof value[Symbol.asyncIterator]==='function');
const sessionSchema=z.object({events:asyncValues,close:callable});
function parsedOrError<T>(schema:z.ZodType<T>,input:unknown):T{const result=schema.safeParse(input);if(!result.success)throw {code:'InvalidInput',path:'PackABI',message:'Pack returned an invalid boundary value'} satisfies ParseError;return result.data;}
function normalizePlugin(raw:z.output<typeof rawPluginSchema>,manifest:StreamManifest|undefined):TransportPlugin{
 switch(raw.kind){
  case 'http':return {kind:'http',execute:async(request,credentials)=>parsedOrError(transportResultSchema,await raw.execute(request,credentials))};
  case 'custom':return {kind:'custom',protocol:raw.protocol,execute:async(request,credentials)=>parsedOrError(transportResultSchema,await raw.execute(request,credentials)),close:async()=>{await raw.close();}};
  case 'ws':return {kind:'ws',open:async(channel,request,credentials)=>{if(!manifest?.channels.some(declared=>declared.name===channel.name&&declared.cursor===channel.cursor&&declared.endReasons.length===channel.endReasons.length&&declared.endReasons.every(reason=>channel.endReasons.includes(reason))))throw {code:'InvalidInput',path:'StreamChannel',message:'Stream channel is not declared by this manifest'} satisfies ParseError;const session=parsedOrError(sessionSchema,await raw.open(channel,request,credentials));return {events:(async function*(){for await(const value of session.events){const item=parsedOrError(streamItemSchema,value);if(item.channel!==channel.name)throw {code:'InvalidInput',path:'StreamItem.channel',message:'Stream channel mismatch'} satisfies ParseError;yield item;}})(),close:async()=>{await session.close();}};}};
 }
}
const rawModuleSchema=z.object({BROKER_PACK_API_VERSION:z.literal(2),declaration:providerDeclarationSchema,translation:z.object({encodeOperation:callable,decodeOperation:callable,decodeReceipt:callable,decodeObservation:callable}),transports:z.array(rawPluginSchema),streamManifest:streamManifestSchema.optional(),recovery:z.record(z.string(),z.record(operationKindSchema,z.object({contract:recoverableWriteContractSchema,recover:callable}))),bindings:z.record(z.string(),z.record(operationKindSchema,rawPluginSchema))});
const operationSchema=z.strictObject({kind:operationKindSchema,key:providerKeySchema,payload:serializableSchema});
function decode<T>(schema:z.ZodType<T>,fn:(...args:unknown[])=>unknown,input:unknown):Result<T,ParseError>{try{return parsedOrError(resultSchema(schema),fn(input));}catch{return {kind:'error',error:{code:'InvalidInput',path:'Translation',message:'Translation failed boundary validation'}};}}
export function validatePackModule(input:unknown,expected?:{readonly providerId:ProviderDeclaration['providerId'];readonly projectionVersion:ProviderDeclaration['projectionVersion']}):LoaderValidationResult{
 const outer=z.object({BROKER_PACK_API_VERSION:z.unknown(),declaration:z.unknown()}).safeParse(input);
 if(!outer.success||outer.data.BROKER_PACK_API_VERSION!==2)return {kind:'invalid',code:'ApiVersion',path:'BROKER_PACK_API_VERSION'};
 const rows=z.object({venues:z.record(z.string(),z.record(z.string(),z.unknown()))}).safeParse(outer.data.declaration);
 if(rows.success)for(const [venue,table] of Object.entries(rows.data.venues))for(const [kind,row] of Object.entries(table)){const write=z.object({direction:z.literal('write'),status:z.object({kind:z.literal('supported')}),contract:writeContractSchema}).safeParse(row);if(write.success&&write.data.contract.idempotency.kind==='none'&&write.data.contract.readByKey.kind==='none')return {kind:'invalid',code:'MissingHandler',path:`recovery.${venue}.${kind}`};}
 if(!serializableSchema.safeParse(outer.data.declaration).success)return {kind:'invalid',code:'NonSerializable',path:'declaration'};
 const declaration=providerDeclarationSchema.safeParse(outer.data.declaration);
 if(!declaration.success)return {kind:'invalid',code:'DeclarationInvalid',path:'declaration'};
 if(expected&&(declaration.data.providerId!==expected.providerId||declaration.data.projectionVersion!==expected.projectionVersion))return {kind:'invalid',code:'ProjectionIdentity',path:'declaration'};
 const raw=rawModuleSchema.safeParse(input);
 if(!raw.success)return {kind:'invalid',code:'MissingHandler',path:raw.error.issues[0]?.path.join('.')??'module'};
 if(!raw.data.streamManifest&&(raw.data.transports.some(plugin=>plugin.kind==='ws')||Object.values(raw.data.bindings).some(table=>Object.values(table).some(plugin=>plugin.kind==='ws'))))return {kind:'invalid',code:'MissingHandler',path:'streamManifest'};
 const recovery:Record<string,Record<string,PlacementRecovery<ProviderTypes>>>={};
 const bindings:Record<string,Record<string,TransportPlugin>>={};
 for(const [venue,table] of Object.entries(raw.data.declaration.venues))for(const [kind,row] of Object.entries(table)){
  if(row.direction!=='write'||row.status.kind!=='supported')continue;
  const key=operationKindSchema.parse(kind),handler=raw.data.recovery[venue]?.[key],binding=raw.data.bindings[venue]?.[key];
  if(!handler||!binding||JSON.stringify(handler.contract)!==JSON.stringify(row.contract))return {kind:'invalid',code:'MissingHandler',path:`recovery.${venue}.${kind}`};
  const venueRecovery=recovery[venue]??(recovery[venue]={});
  venueRecovery[kind]={contract:handler.contract,recover:async(attempt,now)=>parsedOrError(recoveryResultSchema,await handler.recover(attempt,now))};
  const venueBindings=bindings[venue]??(bindings[venue]={});venueBindings[kind]=normalizePlugin(binding,raw.data.streamManifest);
 }
 const translation:Translation<ProviderTypes>={encodeOperation:value=>decode(serializableSchema,raw.data.translation.encodeOperation,value),decodeOperation:value=>decode(operationSchema,raw.data.translation.decodeOperation,value),decodeReceipt:value=>decode(receiptPayloadSchema,raw.data.translation.decodeReceipt,value),decodeObservation:value=>decode(observationPayloadSchema,raw.data.translation.decodeObservation,value)};
 const module:PackModule<ProviderDeclaration>={BROKER_PACK_API_VERSION:2,declaration:raw.data.declaration,translation,transports:raw.data.transports.map(plugin=>normalizePlugin(plugin,raw.data.streamManifest)),recovery,bindings,...(raw.data.streamManifest?{streamManifest:raw.data.streamManifest}:{})};
 return {kind:'loaded',module};
}
