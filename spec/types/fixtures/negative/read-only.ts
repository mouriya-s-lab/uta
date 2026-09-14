// Expected TS2322: a read-only provider cannot construct a write handler.
import type { ReadOnly, WriteHandler } from '../../channel.ts';
const forbidden:WriteHandler<ReadOnly<string>> = async (_value:string)=>'written';
