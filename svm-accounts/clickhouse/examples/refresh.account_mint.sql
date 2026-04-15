-- refreshable --
SYSTEM REFRESH VIEW account_mint;

-- inspect progress --
SELECT *
FROM system.view_refreshes
WHERE view = 'account_mint';