import { z } from 'zod';
import { accountIdSchema,providerIdSchema,projectionVersionSchema,instantSchema,sourceDigestSchema,sha256 } from './ids.ts';
import type { Sha256 } from './ids.ts';
import { decimalStringSchema } from './money.ts';
import { durationSchema } from './time.ts';
import type { Serializable } from './provider/indexed.ts';
import type { Result,ParseError } from './result.ts';
export const MIN_PROVIDER_CONNECTIONS=100;
const nonempty=z.string().trim().min(1);
const count=z.number().int().positive().max(Number.MAX_SAFE_INTEGER);
export const ruleConfigSchema=z.discriminatedUnion('kind',[
 z.strictObject({kind:z.literal('maxNotional'),limit:decimalStringSchema.refine(value=>value!=='0'&&!value.startsWith('-'),'Positive notional required'),currency:z.string().regex(/^[A-Z][A-Z0-9]{1,15}$/)}),
 z.strictObject({kind:z.literal('allowedInstruments'),instruments:z.array(nonempty).min(1).refine(values=>new Set(values).size===values.length,'Duplicate instruments')}),
 z.strictObject({kind:z.literal('maxObservationAge'),milliseconds:durationSchema})
]);
export type RuleConfig=z.output<typeof ruleConfigSchema>;
export const authorizationPolicySchema=z.strictObject({selfApprove:z.array(z.enum(['human','agent','connector','schedule','policy'])),agentDecisionAuthority:z.enum(['binding','recommendation']),limits:z.array(ruleConfigSchema)});
export type AuthorizationPolicy=z.output<typeof authorizationPolicySchema>;
export function parseRuleConfig(input:unknown):Result<RuleConfig,ParseError>{const parsed=ruleConfigSchema.safeParse(input);return parsed.success?{kind:'ok',value:parsed.data}:{kind:'error',error:{code:'InvalidInput',path:'RuleConfig',message:parsed.error.message}};}
export function parseAuthorizationPolicy(input:unknown):Result<AuthorizationPolicy,ParseError>{const parsed=authorizationPolicySchema.safeParse(input);return parsed.success?{kind:'ok',value:parsed.data}:{kind:'error',error:{code:'InvalidInput',path:'AuthorizationPolicy',message:parsed.error.message}};}
export const utaModeSchema=z.enum(['lite','readonly','pro']);
export const utaRuntimeConfigSchema=z.strictObject({
 schemaVersion:z.literal(1),policies:z.array(z.strictObject({accountId:accountIdSchema,policy:authorizationPolicySchema})),rules:z.array(z.strictObject({accountId:accountIdSchema,rules:z.array(ruleConfigSchema)})),
 mode:utaModeSchema.optional(),
 fx:z.strictObject({maxAge:durationSchema}),
 projections:z.array(z.strictObject({accountId:accountIdSchema,providerId:providerIdSchema,projectionVersion:projectionVersionSchema,sourceDigest:sourceDigestSchema})),
 recovery:z.array(z.strictObject({accountId:accountIdSchema,maxUnknownDuration:durationSchema})),
 capacity:z.strictObject({maxProviderConnections:count.min(MIN_PROVIDER_CONNECTIONS),maxConcurrentReads:count,perAccountWriteQueueCapacity:count,fanoutQueueCapacity:count,ledgerSegmentMaxBytes:z.literal(16777216)}),
 retention:z.strictObject({snapshotMaxAge:durationSchema,snapshotMaxBytes:count,diagnosticLogMaxBytes:count,quarantineMaxBytes:count}),updatedAt:instantSchema
}).superRefine((config,context)=>{
 for(const rows of [config.policies,config.rules,config.projections,config.recovery])if(new Set(rows.map(row=>row.accountId)).size!==rows.length)context.addIssue({code:'custom',message:'Duplicate account configuration'});
 const ids=new Set(config.policies.map(row=>row.accountId));
 for(const rows of [config.rules,config.projections,config.recovery])if(rows.length!==ids.size||rows.some(row=>!ids.has(row.accountId)))context.addIssue({code:'custom',message:'Configuration account sets differ'});
});
export type UtaRuntimeConfig=z.output<typeof utaRuntimeConfigSchema>;
export function parseUtaRuntimeConfig(input:unknown):Result<UtaRuntimeConfig,ParseError>{const parsed=utaRuntimeConfigSchema.safeParse(input);return parsed.success?{kind:'ok',value:parsed.data}:{kind:'error',error:{code:'InvalidInput',path:'UtaRuntimeConfig',message:parsed.error.message}};}
function sequence(value:Serializable):value is readonly Serializable[]{return Array.isArray(value);}
export function canonicalJson(value:Serializable):string{
 if(value===null||typeof value!=='object')return JSON.stringify(value);
 if(sequence(value))return '['+value.map(canonicalJson).join(',')+']';
 return '{'+Object.keys(value).sort().map(key=>{const item=value[key];if(item===undefined)throw new Error('Nonserializable canonical input');return JSON.stringify(key)+':'+canonicalJson(item);}).join(',')+'}';
}
export function deriveRuntimeConfigDigest(config:UtaRuntimeConfig):Promise<Sha256>{return sha256(canonicalJson({schemaVersion:config.schemaVersion,...(config.mode===undefined?{}:{mode:config.mode}),fx:config.fx,policies:config.policies,rules:config.rules,projections:config.projections,recovery:config.recovery,capacity:config.capacity,retention:config.retention}));}
