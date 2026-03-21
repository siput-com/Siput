package client

import (
    "encoding/json"
    "fmt"
    "net/http"
    "bytes"
)

type Client struct {
    Endpoint string
    HTTP     *http.Client
}

func New(endpoint string) *Client {
    if endpoint == "" {
        endpoint = "http://localhost:8080"
    }
    return &Client{Endpoint: endpoint, HTTP: &http.Client{}}
}

func (c *Client) GetStatus() (map[string]any, error) {
    resp, err := c.HTTP.Get(fmt.Sprintf("%s/status", c.Endpoint))
    if err != nil {
        return nil, err
    }
    defer resp.Body.Close()

    if resp.StatusCode != http.StatusOK {
        return nil, fmt.Errorf("status %d", resp.StatusCode)
    }

    var result map[string]any
    if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
        return nil, err
    }
    return result, nil
}

func (c *Client) GetBalance(address string) (map[string]any, error) {
    resp, err := c.HTTP.Get(fmt.Sprintf("%s/balance/%s", c.Endpoint, address))
    if err != nil {
        return nil, err
    }
    defer resp.Body.Close()
    var result map[string]any
    if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
        return nil, err
    }
    return result, nil
}

func (c *Client) SendTransaction(tx map[string]any) (map[string]any, error) {
    payload, err := json.Marshal(tx)
    if err != nil {
        return nil, err
    }
    resp, err := c.HTTP.Post(fmt.Sprintf("%s/transaction/send", c.Endpoint), "application/json", bytes.NewReader(payload))
    if err != nil {
        return nil, err
    }
    defer resp.Body.Close()
    var result map[string]any
    if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
        return nil, err
    }
    return result, nil
}
