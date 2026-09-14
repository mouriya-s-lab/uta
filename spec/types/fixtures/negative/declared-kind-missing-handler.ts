// Expected TS2741: every declared supported write kind needs its own placement recovery witness.
import { declaration } from '../positive/supported-without-witness.ts';
import type { PackModule } from '../../provider/abi.ts';
declare const module:PackModule<typeof declaration>;
const forbidden:PackModule<typeof declaration> = {...module,recovery:{paper:{}}};
