package wallet

import (
    "crypto/rand"
    "crypto/sha256"
    "encoding/hex"
    "fmt"
)

type Wallet struct {
    PrivateKey string
    Address    string
}

func Create() (*Wallet, error) {
    raw := make([]byte, 32)
    _, err := rand.Read(raw)
    if err != nil {
        return nil, err
    }
    h := sha256.Sum256(raw)
    return &Wallet{
        PrivateKey: hex.EncodeToString(raw),
        Address:    hex.EncodeToString(h[:])[:40],
    }, nil
}

func SignTransaction(privateKey string, tx map[string]any) (map[string]any, error) {
    // Simplified deterministic signature for SDK sample.
    payload := privateKey
    for k, v := range tx {
        payload += k + ":" + fmt.Sprintf("%v", v) + ";"
    }
    sig := sha256.Sum256([]byte(payload))
    out := map[string]any{}
    for k, v := range tx {
        out[k] = v
    }
    out["signature"] = hex.EncodeToString(sig[:])
    return out, nil
}
