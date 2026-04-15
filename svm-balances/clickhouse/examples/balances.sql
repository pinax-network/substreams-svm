WITH owners AS (
    SELECT owner, account
    FROM `solana:svm-accounts@v0.3.1`.owner_state AS o
    WHERE owner IN ['GXYBNgyYKbSLr938VJCpmGLCUaAHWsncTi7jDoQSdFR9']
),
balances AS (
    SELECT
        max(timestamp) AS timestamp,
        max(block_num) AS block_num,
        account,
        argMax(b.amount, b.block_num) AS amount,
        mint,
        decimals
    FROM `solana:svm-balances@v0.3.3`.balances AS b
    WHERE b.account IN (SELECT account FROM owners)
    GROUP BY b.mint, b.account, b.program_id, b.decimals
    HAVING amount > 0
    ORDER BY timestamp DESC, account, mint
    LIMIT 10
),
mints AS (
    SELECT DISTINCT mint FROM balances
),
decimals AS (
    SELECT mint, decimals
    FROM `solana:svm-accounts@v0.3.1`.decimals_state
    WHERE mint IN mints
    LIMIT 1 BY mint
),
metadata AS (
    SELECT mint, name, symbol, uri
    FROM `solana:svm-metadata@v0.3.3`.metadata
    WHERE mint IN mints
    LIMIT 1 BY mint
)
SELECT
    /* amount */
    b.amount AS amount,
    b.mint AS mint,
    o.owner AS owner,
    b.amount / pow(10, coalesce(b.decimals, d.decimals, 1)) AS value,
    coalesce(b.decimals, d.decimals) AS decimals,

    /* metadata */
    nullIf(m.name, '') AS name,
    nullIf(m.symbol, '') AS symbol,
    nullIf(m.uri, '') AS uri
FROM balances AS b
JOIN owners AS o USING (account)
LEFT JOIN decimals AS d USING (mint)
LEFT JOIN metadata AS m USING (mint)
ORDER BY b.timestamp DESC, b.account, b.mint
