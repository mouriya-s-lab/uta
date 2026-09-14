// Expected TS2322: a real Decimal instance is not a serializable ABI payload.
import type { TransportRequest } from '../../provider/abi.ts';
import { Decimal } from 'decimal.js';
declare const request:TransportRequest;
const forbidden:TransportRequest = {...request,payload:new Decimal('1')};
