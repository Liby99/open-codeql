package com.test;

import java.io.*;
import java.sql.*;

/**
 * Security anti-patterns — known vulnerable code for security query testing.
 * Each method demonstrates a specific CWE vulnerability.
 */
public class SecurityPatterns {

    // CWE-89: SQL Injection
    public ResultSet unsafeQuery(Connection conn, String userInput) throws SQLException {
        String query = "SELECT * FROM users WHERE name = '" + userInput + "'";
        Statement stmt = conn.createStatement();
        return stmt.executeQuery(query);
    }

    // CWE-89: Safe version (parameterized)
    public ResultSet safeQuery(Connection conn, String userInput) throws SQLException {
        String query = "SELECT * FROM users WHERE name = ?";
        PreparedStatement stmt = conn.prepareStatement(query);
        stmt.setString(1, userInput);
        return stmt.executeQuery();
    }

    // CWE-78: Command Injection
    public void unsafeExec(String userInput) throws IOException {
        Runtime.getRuntime().exec("ls " + userInput);
    }

    // CWE-78: Another form
    public void unsafeProcessBuilder(String filename) throws IOException {
        ProcessBuilder pb = new ProcessBuilder("cat", filename);
        pb.start();
    }

    // CWE-22: Path Traversal
    public String readFile(String userPath) throws IOException {
        File f = new File("/data/" + userPath);
        BufferedReader reader = new BufferedReader(new FileReader(f));
        StringBuilder sb = new StringBuilder();
        String line;
        while ((line = reader.readLine()) != null) {
            sb.append(line).append("\n");
        }
        reader.close();
        return sb.toString();
    }

    // CWE-676: Use of dangerous function (gets equivalent)
    @Deprecated
    public static String readPassword() throws IOException {
        BufferedReader reader = new BufferedReader(new InputStreamReader(System.in));
        return reader.readLine(); // No masking
    }

    // CWE-327: Weak cryptography (conceptual — just string manipulation)
    public static String weakHash(String input) {
        int hash = 0;
        for (int i = 0; i < input.length(); i++) {
            hash = hash * 31 + input.charAt(i);
        }
        return Integer.toHexString(hash);
    }

    // CWE-190: Integer Overflow
    public static int unsafeMultiply(int a, int b) {
        return a * b; // No overflow check
    }

    // CWE-476: Null pointer dereference risk
    public String unsafeNullDeref(String input) {
        return input.trim().toUpperCase();
    }

    // CWE-561: Dead code
    public int deadCode(int x) {
        if (x > 0) {
            return x;
        } else {
            return -x;
        }
        // Dead code below (unreachable)
    }

    // CWE-835: Infinite loop risk
    public void riskyLoop(int n) {
        int i = 0;
        while (i < n) {
            System.out.println(i);
            // Missing: i++ (potential infinite loop if n > 0)
        }
    }

    // Empty catch block (bad practice)
    public void swallowException(String filename) {
        try {
            FileReader fr = new FileReader(filename);
            fr.close();
        } catch (IOException e) {
            // Empty catch — swallows exception
        }
    }

    // Resource leak
    public String leakyRead(String path) throws IOException {
        FileReader fr = new FileReader(path);
        BufferedReader br = new BufferedReader(fr);
        return br.readLine();
        // Never closed
    }

    public static void main(String[] args) throws Exception {
        SecurityPatterns sp = new SecurityPatterns();
        System.out.println(weakHash("password"));
        System.out.println(unsafeMultiply(Integer.MAX_VALUE, 2));
    }
}
