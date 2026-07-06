module github.com/ce-net/ce-conformance/runners/go

go 1.26.4

require (
	github.com/ce-net/ce-go v0.0.0
	github.com/ce-net/economy-adapter/clients/go v0.0.0
)

replace github.com/ce-net/ce-go => ../../../ce-go

replace github.com/ce-net/economy-adapter/clients/go => ../../../economy-adapter/clients/go
