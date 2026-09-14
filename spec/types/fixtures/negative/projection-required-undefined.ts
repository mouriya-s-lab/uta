// Expected TS2345: required undefined is not an omitted optional JSON property.
import { z } from 'zod';
import { readProjectionResponseSchema } from '../../wire/projection.ts';
readProjectionResponseSchema(z.strictObject({observed:z.undefined()}));
