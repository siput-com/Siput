# Siput Go SDK

## Setup

cd sdk/go

go test ./...

## Example

package main

import (
    "fmt"
    "github.com/siput/sdk-go/client"
    "github.com/siput/sdk-go/wallet"
)

func main() {
    w, _ := wallet.Create()
    fmt.Println("Wallet:", w.Address)

    c := client.New("http://localhost:8080")
    status, _ := c.GetStatus()
    fmt.Println("Status:", status)
}
