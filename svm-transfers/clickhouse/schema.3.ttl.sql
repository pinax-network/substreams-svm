ALTER TABLE spl_transfer TTL timestamp + INTERVAL 1 DAY;
ALTER TABLE system_transfer TTL timestamp + INTERVAL 1 DAY;
ALTER TABLE system_transfer_with_seed TTL timestamp + INTERVAL 1 DAY;
ALTER TABLE system_withdraw_nonce_account TTL timestamp + INTERVAL 1 DAY;