import { schemaFromParser } from './ids.ts';
import type { Brand } from './ids.ts';
import type { Result, ParseError } from './result.ts';
export type DecimalString = Brand<string,'DecimalString'>;
export type Money = { readonly amount: DecimalString; readonly currency: string };
export type Qty = { readonly amount: DecimalString; readonly unit: string };
export function parseDecimalString(input: unknown, maxScale = 18): Result<DecimalString,ParseError> {
  if (!Number.isSafeInteger(maxScale) || maxScale < 0 || typeof input !== 'string' || !/^-?(0|[1-9][0-9]*)(\.[0-9]*[1-9])?$/.test(input) || input === '-0' || (input.split('.')[1]?.length ?? 0) > maxScale) return {kind:'error',error:{code:'InvalidInput',path:'DecimalString',message:'Expected canonical finite decimal string within scale'}};
  // PARSER BOUNDARY: canonical grammar and scale verified; no numeric conversion.
  return {kind:'ok',value:input as DecimalString};
}
export function parseMoney(input: unknown, maxScale = 18): Result<Money,ParseError> {
  if (typeof input !== 'object' || input === null || !('amount' in input) || !('currency' in input) || typeof input.currency !== 'string' || !/^[A-Z][A-Z0-9]{1,15}$/.test(input.currency)) return {kind:'error',error:{code:'InvalidInput',path:'Money',message:'Expected amount and currency'}};
  const amount = parseDecimalString(input.amount,maxScale);
  return amount.kind === 'error' ? amount : {kind:'ok',value:{amount:amount.value,currency:input.currency}};
}
export function parseQty(input: unknown, maxScale = 18): Result<Qty,ParseError> {
  if (typeof input !== 'object' || input === null || !('amount' in input) || !('unit' in input) || typeof input.unit !== 'string' || input.unit.trim().length === 0) return {kind:'error',error:{code:'InvalidInput',path:'Qty',message:'Expected amount and unit'}};
  const amount = parseDecimalString(input.amount,maxScale);
  return amount.kind === 'error' ? amount : {kind:'ok',value:{amount:amount.value,unit:input.unit}};
}

export const decimalStringSchema=schemaFromParser(parseDecimalString);
export const moneySchema=schemaFromParser(parseMoney);
export const qtySchema=schemaFromParser(parseQty);
