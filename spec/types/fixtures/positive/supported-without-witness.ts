import { defineDeclaration } from '../../provider/declaration.ts';
import { validatePackModule,streamItemSchema,streamChannelSchema,transportResultSchema } from '../../provider/abi.ts';
import type { PackModule,HTTPExecutor } from '../../provider/abi.ts';
import { providerIdSchema,projectionVersionSchema,sourceDigestSchema,sha256,operationKindSchema,instantSchema } from '../../ids.ts';
import { durationSchema } from '../../time.ts';
import { Decimal } from 'decimal.js';
function check(condition:boolean,label:string):void{if(!condition)throw new Error(label);}
export const declaration=defineDeclaration({
 providerId:providerIdSchema.parse('model-loader'),projectionVersion:projectionVersionSchema.parse('2'),sourceDigest:sourceDigestSchema.parse(await sha256('synthetic loader conformance fixture')),
 venues:{paper:{'order.place':{direction:'write',status:{kind:'supported'},contract:{idempotency:{kind:'cachedReplay',keyField:'clientKey',scope:'account',retention:durationSchema.parse(1000)},readByKey:{kind:'none'}}}}},transportFamily:['http'],auth:[]
});
const unavailable={kind:'error',error:{code:'InvalidInput',path:'model',message:'No provider response in this local loader fixture'}} as const;
const http:HTTPExecutor={kind:'http',execute:async()=>({kind:'responded',status:200,rawBody:'{}',payload:{}})};
const module:PackModule<typeof declaration>={BROKER_PACK_API_VERSION:2,declaration,translation:{encodeOperation:value=>({kind:'ok',value:value.payload}),decodeOperation:()=>unavailable,decodeReceipt:()=>unavailable,decodeObservation:()=>unavailable},transports:[http],recovery:{paper:{'order.place':{contract:declaration.venues.paper['order.place'].contract,recover:async()=>({kind:'stillUnknown',nextCheckAfter:instantSchema.parse(50),reason:'Local fixture has no upstream'})}}},bindings:{paper:{'order.place':http}}};
const missing=validatePackModule({...module,recovery:{}});
check(missing.kind==='invalid'&&missing.code==='MissingHandler','Supported write without witness is rejected by the real loader');
const noneNone=validatePackModule({...module,declaration:{...declaration,venues:{paper:{'order.place':{direction:'write',status:{kind:'supported'},contract:{idempotency:{kind:'none'},readByKey:{kind:'none'}}}}}}});
check(noneNone.kind==='invalid'&&noneNone.code==='MissingHandler','none/none runtime mirror cannot load');
const loaded=validatePackModule(module);
check(loaded.kind==='loaded','Valid supported declaration and witness load');
if(loaded.kind==='loaded'){
 const plugin=loaded.module.transports[0];
 if(!plugin||plugin.kind!=='http')throw new Error('Expected loaded HTTP transport');
 const result=await plugin.execute({kind:operationKindSchema.parse('order.place'),payload:{},deadline:durationSchema.parse(50),control:{}},[]);
 check(result.kind==='responded','Loaded HTTP plugin validates a real transport result');
}
const wrong=validatePackModule(module,{providerId:providerIdSchema.parse('other'),projectionVersion:declaration.projectionVersion});
check(wrong.kind==='invalid'&&wrong.code==='ProjectionIdentity','Registry identity is checked');
check(validatePackModule({...module,BROKER_PACK_API_VERSION:1}).kind==='invalid','Legacy ABI cannot load');
check(!streamItemSchema.safeParse({kind:'gap',channel:'order.update',reason:'cursorExpired',payload:{},asOf:1}).success,'Gap cannot carry observation facts');
check(streamItemSchema.safeParse({kind:'event',channel:'order.update',asOf:1,payload:{filledQty:'1'}}).success,'Event carries timestamped provider fact payload');
check(streamItemSchema.safeParse({kind:'ended',channel:'order.update',reason:'drain'}).success,'Drain is an ended marker');
check(!transportResultSchema.safeParse({kind:'responded',status:200,rawBody:'{}',payload:new Decimal('1')}).success,'Transport cannot return Decimal instances');
check(!transportResultSchema.safeParse({kind:'unknown',cause:'timeout'}).success,'Unknown requires send evidence');
const channel=streamChannelSchema.parse({name:'order.update',cursor:'snapshotOnly',endReasons:['drain']});
let closed=false;
const ws={kind:'ws',open:async()=>({events:(async function*(){yield {kind:'event',channel:channel.name,asOf:1,payload:{filledQty:'1'}};yield {kind:'gap',channel:channel.name,reason:'providerReset'};yield {kind:'ended',channel:channel.name,reason:'drain'};})(),close:async()=>{closed=true;}})};
const noManifest=validatePackModule({...module,transports:[ws]});
check(noManifest.kind==='invalid'&&noManifest.code==='MissingHandler','WebSocket executor requires its declared manifest');
const withStream=validatePackModule({...module,transports:[ws],streamManifest:{version:1,channels:[channel]}});
if(withStream.kind!=='loaded')throw new Error('Valid manifest must load');
const streamPlugin=withStream.module.transports[0];
if(!streamPlugin||streamPlugin.kind!=='ws')throw new Error('Expected WS plugin');
const request={kind:operationKindSchema.parse('order.observe'),payload:{},deadline:durationSchema.parse(50),control:{}};
const session=await streamPlugin.open(channel,request,[]);
const received:string[]=[];
for await(const item of session.events)received.push(item.kind);
await session.close();
check(received.join(',')==='event,gap,ended'&&closed,'Loaded stream preserves fact/gap/end distinctions and closes its session');
let undeclaredRejected=false;
try{await streamPlugin.open({...channel,name:operationKindSchema.parse('other.update')},request,[]);}catch{undeclaredRejected=true;}
check(undeclaredRejected,'Undeclared stream channel is rejected before transport open');
console.log('PASS loader: supported witness, none/none mirror, identity, transport and stream boundaries');
