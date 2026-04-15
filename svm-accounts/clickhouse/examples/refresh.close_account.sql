-- refreshable --
SYSTEM REFRESH VIEW close_account_refresh;

-- inspect progress --
SELECT *
FROM system.view_refreshes
WHERE view = 'close_account_refresh';