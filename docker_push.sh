#!/bin/bash
docker build -f docker/ubuntu/Dockerfile --tag kodebox/codechain:$TRAVIS_COMMIT .
echo "$DOCKER_PASSWORD" | docker login -u "$DOCKER_USERNAME" --password-stdin
docker push kodebox/codechain:$TRAVIS_COMMIT
