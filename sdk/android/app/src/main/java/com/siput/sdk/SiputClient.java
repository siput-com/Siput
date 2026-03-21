package com.siput.sdk;

import okhttp3.*;
import java.io.IOException;

public class SiputClient {
    private final String endpoint;
    private final OkHttpClient httpClient;

    public SiputClient(String endpoint) {
        this.endpoint = endpoint.endsWith("/") ? endpoint.substring(0, endpoint.length() - 1) : endpoint;
        this.httpClient = new OkHttpClient();
    }

    public String getStatus() throws IOException {
        Request request = new Request.Builder().url(endpoint + "/status").get().build();
        try (Response response = httpClient.newCall(request).execute()) {
            if (!response.isSuccessful()) throw new IOException("Unexpected code " + response);
            return response.body().string();
        }
    }
}
