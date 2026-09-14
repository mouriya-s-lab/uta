// Expected TS2345: adding work.requested requires explicit consumption handling.
import type { LedgerEntry } from '../../ledger/entries.ts';
import { assertNever } from '../../result.ts';
export function fold(entry:LedgerEntry):void {
 switch(entry.kind){
 case 'intent.proposed':case 'authorization.decided':case 'intent.expired':case 'attempt.started':case 'attempt.abandoned':case 'receipt.recorded':case 'recovery.resolved':case 'observation.recorded':case 'reversal.appended':return;
 }
 assertNever(entry);
}
