import { getFxRatesResponseSchema,searchContractsResponseSchema,ROUTES } from '../../wire/routes.ts';

type Assert<T extends true> = T;
type EveryRouteHasResponse<T extends readonly {readonly response?: unknown}[]> = Exclude<T[number], {readonly response: unknown}> extends never ? true : false;
type EveryReadWriteHasRequest<T extends readonly {readonly channel: string; readonly request?: unknown}[]> = Exclude<Extract<T[number], {readonly channel: 'read-write'}>, {readonly request: unknown}> extends never ? true : false;
type EveryParameterizedHasParams<T extends readonly {readonly path: string; readonly params?: unknown}[]> = Exclude<Extract<T[number], {readonly path: `${string}{${string}}${string}`}>, {readonly params: unknown}> extends never ? true : false;
const responses:Assert<EveryRouteHasResponse<typeof ROUTES>>=true;
const requests:Assert<EveryReadWriteHasRequest<typeof ROUTES>>=true;
const parameters:Assert<EveryParameterizedHasParams<typeof ROUTES>>=true;
const routeCount:41=ROUTES.length;
void responses;void requests;void parameters;void routeCount;
function check(condition:boolean,label:string):void{if(!condition)throw new Error(label);}
const rate={base:'USD',quote:'EUR',rate:'0.9',asOf:1000,source:'fx-service'};
const fx={source:'fx-service',asOf:1000,freshness:{kind:'fresh'},value:{rates:[rate]}};
check(getFxRatesResponseSchema.safeParse(fx).success,'Fresh FX response contains branded decimal rate/source/time');
check(getFxRatesResponseSchema.safeParse({...fx,freshness:{kind:'stale',age:60001}}).success,'Stale FX remains available with explicit positive age');
check(!getFxRatesResponseSchema.safeParse({...fx,freshness:{kind:'stale'}}).success,'Stale FX cannot omit its age');
check(!getFxRatesResponseSchema.safeParse({...fx,value:{rates:[{...rate,rate:0.9}]}}).success,'FX financial values reject floating-point JSON numbers');
check(!getFxRatesResponseSchema.safeParse({...fx,value:{rates:[{base:'USD',quote:'EUR',rate:'0.9'}]}}).success,'FX observations cannot omit source or asOf');
const hit={aliceId:'US/AAPL',nativeKey:'AAPL',source:'alpaca',contract:{symbol:'AAPL',secType:'STK'},derivativeSecTypes:[]};
const search={source:'catalogue',asOf:1000,freshness:{kind:'fresh',asOf:1000,maxAge:60000},value:{results:[hit],count:1}};
check(searchContractsResponseSchema.safeParse(search).success,'Contract search preserves cross-provider identity and provider native key');
check(!searchContractsResponseSchema.safeParse({...search,value:{results:[{nativeKey:'AAPL',source:'alpaca',contract:{symbol:'AAPL'},derivativeSecTypes:[]}],count:1}}).success,'Provider symbol alone cannot replace aliceId');
console.log('PASS routes: FX decimal/provenance/freshness and contract identity boundaries');
