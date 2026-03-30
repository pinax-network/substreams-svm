-- OpenBook Fill --
CREATE TABLE IF NOT EXISTS openbook_fill AS BASE_EVENTS
COMMENT 'OpenBook Fill Log';
ALTER TABLE openbook_fill
    ADD COLUMN IF NOT EXISTS market       String COMMENT 'Market account',
    ADD COLUMN IF NOT EXISTS maker        String COMMENT 'Maker account',
    ADD COLUMN IF NOT EXISTS taker        String COMMENT 'Taker account',
    ADD COLUMN IF NOT EXISTS price        Int64 COMMENT 'Price',
    ADD COLUMN IF NOT EXISTS quantity     Int64 COMMENT 'Quantity',
    ADD COLUMN IF NOT EXISTS taker_side   UInt32 COMMENT 'Taker side',
    ADD COLUMN IF NOT EXISTS seq_num      UInt64 COMMENT 'Sequence number';

-- OpenBook Total Order Fill --
CREATE TABLE IF NOT EXISTS openbook_total_order_fill AS BASE_EVENTS
COMMENT 'OpenBook Total Order Fill';
ALTER TABLE openbook_total_order_fill
    ADD COLUMN IF NOT EXISTS taker                    String COMMENT 'Taker account',
    ADD COLUMN IF NOT EXISTS side                     UInt32 COMMENT 'Side',
    ADD COLUMN IF NOT EXISTS total_quantity_paid       UInt64 COMMENT 'Total quantity paid',
    ADD COLUMN IF NOT EXISTS total_quantity_received   UInt64 COMMENT 'Total quantity received',
    ADD COLUMN IF NOT EXISTS fees                     UInt64 COMMENT 'Fees';
