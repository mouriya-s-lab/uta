// Expected TS2322: an unresolved read/write channel union cannot produce a write handler.
import type { ReadOnly,ReadWrite,WriteHandler } from '../../channel.ts';
const forbidden:WriteHandler<ReadOnly<string>|ReadWrite<string,string>> = async(value:string)=>value;
