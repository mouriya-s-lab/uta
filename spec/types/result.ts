import { z } from 'zod';
export type Result<T, E> = { readonly kind: 'ok'; readonly value: T } | { readonly kind: 'error'; readonly error: E };
export type ParseError = { readonly code: 'InvalidInput'; readonly path: string; readonly message: string };
export function assertNever(value: never): never { throw new Error(`Unreachable variant: ${String(value)}`); }
export function unwrap<T, E>(result: Result<T, E>): T {
  if (result.kind === 'error') throw new Error(JSON.stringify(result.error));
  return result.value;
}
export const parseErrorSchema=z.strictObject({code:z.literal('InvalidInput'),path:z.string(),message:z.string().min(1)});
export function resultSchema<T>(valueSchema:z.ZodType<T>) {
 return z.discriminatedUnion('kind',[z.strictObject({kind:z.literal('ok'),value:valueSchema}),z.strictObject({kind:z.literal('error'),error:parseErrorSchema})]);
}
