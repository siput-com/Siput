package com.siput.sdk;

import java.security.SecureRandom;
import java.security.MessageDigest;
import java.util.Formatter;

public class SiputWallet {
    public static class Wallet {
        public final String privateKey;
        public final String address;

        public Wallet(String privateKey, String address) {
            this.privateKey = privateKey;
            this.address = address;
        }
    }

    public static Wallet create() throws Exception {
        byte[] secret = new byte[32];
        SecureRandom.getInstanceStrong().nextBytes(secret);
        String pk = bytesToHex(secret);
        String address = sha256Hex(secret).substring(0, 40);
        return new Wallet(pk, address);
    }

    private static String sha256Hex(byte[] data) throws Exception {
        MessageDigest md = MessageDigest.getInstance("SHA-256");
        byte[] digest = md.digest(data);
        return bytesToHex(digest);
    }

    private static String bytesToHex(byte[] bytes) {
        Formatter formatter = new Formatter();
        for (byte b : bytes) {
            formatter.format("%02x", b);
        }
        String result = formatter.toString();
        formatter.close();
        return result;
    }
}
