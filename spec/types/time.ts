import { z } from 'zod';
import type { Brand } from './ids.ts';
import { asOfSchema,schemaFromParser } from './ids.ts';
import type { Result,ParseError } from './result.ts';
export type { Instant,AsOf } from './ids.ts';
export { parseInstant,parseAsOf,instantSchema,asOfSchema } from './ids.ts';
export type Duration=Brand<number,'Duration'>;
export function parseDuration(input:unknown):Result<Duration,ParseError>{
 if(typeof input!=='number'||!Number.isSafeInteger(input)||input<=0)return {kind:'error',error:{code:'InvalidInput',path:'Duration',message:'Expected positive safe integer milliseconds'}};
 // PARSER BOUNDARY: finite positive duration verified.
 return {kind:'ok',value:input as Duration};
}
export const durationSchema=schemaFromParser(parseDuration);
export const freshnessSchema=z.discriminatedUnion('kind',[z.strictObject({kind:z.literal('fresh'),asOf:asOfSchema,maxAge:durationSchema}),z.strictObject({kind:z.literal('stale'),asOf:asOfSchema,maxAge:durationSchema}),z.strictObject({kind:z.literal('missing'),reason:z.string().min(1)})]);
export type Freshness=z.output<typeof freshnessSchema>;
