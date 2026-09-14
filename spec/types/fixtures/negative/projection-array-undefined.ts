// Expected TS2345: undefined array elements cannot silently encode as null.
import { z } from 'zod';
import { readProjectionResponseSchema } from '../../wire/projection.ts';
readProjectionResponseSchema(z.array(z.union([z.string(),z.undefined()])));
