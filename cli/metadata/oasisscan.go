package metadata

import (
	"context"
	"encoding/json"
	"fmt"
	"io/ioutil"
	"net/http"
	"time"

	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

type oScanResponse struct {
	Data *oScanData `json:"data"`
}

type oScanData struct {
	List []*oScanVal `json:"list"`
}

type oScanVal struct {
	Rank          uint64          `json:"rank"`
	EntityID      string          `json:"entityId"`
	EntityAddress staking.Address `json:"entityAddress"`
	NodeID        string          `json:"nodeId"`
	Name          string          `json:"name"`
}

// EntitiesFromOasisscan queries oasisscan for all known entities.
func EntitiesFromOasisscan(ctx context.Context) (map[types.Address]*Entity, error) {
	reqCtx, cancel := context.WithTimeout(ctx, time.Second*5)
	defer cancel()

	req, err := http.NewRequestWithContext(reqCtx, "GET", "https://www.oasisscan.com/mainnet/validator/list", nil)
	if err != nil {
		return nil, err
	}
	client := &http.Client{}
	resp, err := client.Do(req)
	if err != nil {
		return nil, err
	}
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("invalid response status: %d", resp.StatusCode)
	}
	if resp == nil {
		return nil, fmt.Errorf("no response")
	}
	if resp != nil {
		defer resp.Body.Close()
	}
	vals, err := ioutil.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response body: %w", err)
	}
	var oResp oScanResponse
	if err = json.Unmarshal(vals, &oResp); err != nil {
		return nil, err
	}

	entities := make(map[types.Address]*Entity, len(oResp.Data.List))
	for _, dl := range oResp.Data.List {
		var pk signature.PublicKey
		if err = pk.UnmarshalText([]byte(dl.EntityID)); err != nil {
			fmt.Println("oasisscan invalid entity ID: ", pk)
			continue
		}
		em := &Entity{
			ID:   pk,
			Name: dl.Name,
		}
		entities[em.Address()] = em
	}

	return entities, nil
}
