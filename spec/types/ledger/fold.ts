import type { IntentId, ProposalId, EntryId, AsOf, EntryPosition } from '../ids.ts';
import { parseDecimalString } from '../money.ts';
import type { LedgerEntry, EntryOf } from './entries.ts';
import type { OrderState, ObservationEnvelope, Serializable } from '../provider/indexed.ts';
import { assertNever } from '../result.ts';
import type { ParseError } from '../result.ts';
export type IntentOutcome = 'proposed'|'authorized'|'withdrawn'|'rejected'|'expired'|'attempting'|'accepted'|'rejectedByProvider'|'unknown'|'awaitingReview'|'filled'|'partiallyFilled'|'cancelled'|'abandoned';
export type OrderProjection = {readonly kind:'missing'} | {readonly kind:'unconsumed';readonly reason:ParseError} | {readonly kind:'observed';readonly asOf:AsOf;readonly position:EntryPosition;readonly state:OrderState;readonly observation:ObservationEnvelope;readonly observationConflict:boolean};
export type ProposalOutcome = {readonly proposalId:ProposalId;readonly members:readonly {readonly intentId:IntentId;readonly outcome:IntentOutcome}[]};
function stateOf(entry:EntryOf<'observation.recorded'>):OrderState | undefined {
  const state = entry.payload.observation.payload.state;
  if(typeof state !== 'object' || state === null || !('kind' in state) || !('filledQty' in state)) return undefined;
  const qty = parseDecimalString(state.filledQty);
  if(qty.kind === 'error') return undefined;
  const kind = state.kind;
  if(kind !== 'open' && kind !== 'filled' && kind !== 'partiallyFilled' && kind !== 'cancelled') return undefined;
  return {kind,filledQty:qty.value};
}
function isSequence(value:Serializable):value is readonly Serializable[] { return Array.isArray(value); }
function sameState(left:Serializable,right:Serializable):boolean {
  if(left === right) return true;
  if(left === null || right === null || typeof left !== 'object' || typeof right !== 'object') return false;
  if(isSequence(left) || isSequence(right)) {
    if(!isSequence(left) || !isSequence(right) || left.length !== right.length) return false;
    return left.every((value,index)=>{const other = right[index];return other !== undefined && sameState(value,other);});
  }
  const keys = Object.keys(left);
  return keys.length === Object.keys(right).length && keys.every(key=>{
    const value = left[key], other = right[key];
    return value !== undefined && other !== undefined && sameState(value,other);
  });
}
export function orderProjection(providerRef:string,entries:readonly LedgerEntry[]):OrderProjection {
  let projection:OrderProjection = {kind:'missing'};
  for(const entry of entries) {
    if(entry.kind !== 'observation.recorded' || entry.payload.observation.payload.providerRef !== providerRef) continue;
    const state = stateOf(entry);
    if(!state) return {kind:'unconsumed',reason:{code:'InvalidInput',path:`entries.${entry.position}.observation.state`,message:'Order projection cannot decode provider-local observation state'}};
    const asOf = entry.payload.observation.payload.asOf;
    if(projection.kind === 'missing' || asOf > projection.asOf) projection = {kind:'observed',asOf,position:entry.position,state,observation:entry.payload.observation,observationConflict:projection.kind === 'observed' && projection.observationConflict};
    else if(asOf === projection.asOf && !sameState(entry.payload.observation.payload.state,projection.observation.payload.state)) projection = {kind:'observed',asOf,position:entry.position,state,observation:entry.payload.observation,observationConflict:true};
  }
  return projection;
}
export function intentOutcome(intentId:IntentId,entries:readonly LedgerEntry[]):IntentOutcome {
  let outcome:IntentOutcome = 'proposed';
  let started = false;
  let proposalId:ProposalId|undefined;
  let providerRef:string|undefined;
  const targets = new Set<EntryId>();
  for(const entry of entries) {
    switch(entry.kind) {
      case 'intent.proposed': if(entry.payload.intentId === intentId) {proposalId = entry.payload.proposalId;targets.add(entry.entryId);} break;
      case 'authorization.decided': {
        const scope = entry.payload.scope;
        const applies = scope.kind === 'intents' ? scope.intentIds.includes(intentId) : proposalId === scope.proposalId;
        if(applies) targets.add(entry.entryId);
        if(!applies || entry.payload.policyResult.kind !== 'binding' || started || outcome === 'expired') break;
        switch(entry.payload.action) {
          case 'approve': outcome = 'authorized'; break;
          case 'reject': outcome = 'rejected'; break;
          case 'withdraw': outcome = 'withdrawn'; break;
        }
        break;
      }
      case 'intent.expired': if(entry.payload.intentId === intentId && !started) outcome = 'expired'; break;
      case 'attempt.started': if(entry.payload.intentId === intentId) {started = true;outcome = 'attempting';targets.add(entry.entryId);} break;
      case 'attempt.abandoned': if(entry.payload.intentId === intentId) outcome = 'abandoned'; break;
      case 'receipt.recorded': if(entry.payload.intentId === intentId) {
        const receipt = entry.payload.receipt.payload;
        switch(receipt.kind) {
          case 'accepted': outcome = 'accepted';providerRef = receipt.providerRef;break;
          case 'rejected': outcome = 'rejectedByProvider';break;
          case 'unknown': outcome = 'unknown';break;
        }
      } break;
      case 'recovery.resolved': if(entry.payload.intentId === intentId) {
        const result = entry.payload.result;
        switch(result.kind) {
          case 'found': break; // Only referenced receipt.recorded and observation.recorded establish upstream facts.
          case 'confirmedAbsent': break; // Abandonment requires its own attempt.abandoned entry.
          case 'ambiguous': outcome = 'awaitingReview';break;
          case 'stillUnknown': break; // Unknown status comes from receipt.recorded, including processRestart.
          case 'manual': if(result.resolution === 'awaitingReview') outcome = 'awaitingReview'; else if(result.resolution === 'abandon') outcome = 'abandoned';break;
        }
      } break;
      case 'observation.recorded': break;
      case 'reversal.appended': if(!started && targets.has(entry.payload.target) && entry.payload.consumer === 'Execution') outcome = 'rejected';break;
      case 'work.requested': if(entry.payload.intentId === intentId && (entry.payload.kind === 'review.unknownOutcome' || entry.payload.kind === 'review.ambiguousRecovery')) outcome = 'awaitingReview';break;
      default: assertNever(entry);
    }
  }
  if(providerRef) {
    const order = orderProjection(providerRef,entries);
    if(order.kind === 'observed' && order.state.kind !== 'open') outcome = order.state.kind;
  }
  return outcome;
}
export function proposalOutcome(proposalId:ProposalId,entries:readonly LedgerEntry[]):ProposalOutcome {
  const ids = new Set<IntentId>();
  for(const entry of entries) if(entry.kind === 'intent.proposed' && entry.payload.proposalId === proposalId) ids.add(entry.payload.intentId);
  return {proposalId,members:[...ids].map(intentId=>({intentId,outcome:intentOutcome(intentId,entries)}))};
}
export function validateReversal(target:EntryId,entries:readonly LedgerEntry[]):{readonly kind:'allowed'}|{readonly kind:'rejected';readonly code:'ReversalTargetAlreadyExecuted'|'InvalidRequest'} {
  const entry = entries.find(item=>item.entryId === target);
  if(!entry) return {kind:'rejected',code:'InvalidRequest'};
  let ids:readonly IntentId[] = entry.correlation.intentId ? [entry.correlation.intentId] : [];
  if(entry.kind === 'intent.proposed') ids = [entry.payload.intentId];
  if(entry.kind === 'authorization.decided') {
    const scope = entry.payload.scope;
    ids = scope.kind === 'intents' ? scope.intentIds : entries.flatMap(item=>item.kind === 'intent.proposed' && item.payload.proposalId === scope.proposalId ? [item.payload.intentId] : []);
  }
  if(entries.some(item=>item.kind === 'attempt.started' && ids.includes(item.payload.intentId))) return {kind:'rejected',code:'ReversalTargetAlreadyExecuted'};
  return {kind:'allowed'};
}
