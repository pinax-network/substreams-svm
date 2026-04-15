-- refreshable --
SYSTEM REFRESH VIEW account_mint_refresh;

-- inspect progress --
SELECT *
FROM system.view_refreshes
WHERE view = 'account_mint_refresh';