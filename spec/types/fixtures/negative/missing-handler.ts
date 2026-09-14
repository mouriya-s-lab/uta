// Expected TS2741: every declared capability needs its own handler.
import type { Handlers,ReadOnly } from '../../channel.ts';
const missing:Handlers<{quote:ReadOnly<string>;balances:ReadOnly<number>}> = {quote:{read:async()=> '1'}};
