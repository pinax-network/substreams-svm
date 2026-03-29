ALTER TABLE spl_transfer MODIFY TTL timestamp + INTERVAL 1 DAY;
ALTER TABLE system_transfer MODIFY TTL timestamp + INTERVAL 1 DAY;
ALTER TABLE system_transfer_with_seed MODIFY TTL timestamp + INTERVAL 1 DAY;
ALTER TABLE system_withdraw_nonce_account MODIFY TTL timestamp + INTERVAL 1 DAY;
