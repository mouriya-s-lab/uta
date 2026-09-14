import type { AsOf,Source } from './ids.ts';
import type { Freshness } from './time.ts';
export type Heartbeat={readonly source:Source;readonly asOf:AsOf;readonly freshness:Freshness};
export type ReadOnly<T> = {readonly direction:'read';readonly value:T};
export type ReadWrite<E,T> = {readonly direction:'write';readonly effect:E;readonly value:T};
export type Handler<C> = [C] extends [ReadOnly<infer T>] ? {readonly read:()=>Promise<T>;readonly write?:never} : [C] extends [ReadWrite<infer E,infer T>] ? {readonly read:()=>Promise<T>;readonly write:(effect:E)=>Promise<T>} : never;
export type Handlers<Spec> = {readonly [K in keyof Spec]:Handler<Spec[K]>};
export type WriteHandler<C> = [C] extends [ReadWrite<infer E,infer T>] ? (effect:E)=>Promise<T> : never;
