FROM alpine:3.16
LABEL MAINTAINER="tttlkkkl <tttlkkkl@scale.com>"
ENV TZ "Asia/Shanghai"
COPY target/release/scale /usr/local/bin/scale
WORKDIR /app
EXPOSE 3000
CMD scale