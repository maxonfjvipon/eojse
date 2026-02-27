<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" id="application" version="2.0">
  <xsl:output encoding="UTF-8" method="xml"/>
  <!-- DISPATCH -->
  <xsl:template match="o[@base and not(starts-with(@base, '.')) and o]">
    <xsl:apply-templates select="." mode="app"/>
  </xsl:template>
  <xsl:template match="o[@base and starts-with(@base, '.') and count(o)&gt;1]">
    <application>
      <xsl:if test="@name">
        <xsl:attribute name="name" select="@name"/>
      </xsl:if>
      <xsl:if test="o[last()]/@as">
        <xsl:attribute name="as" select="o[last()]/@as"/>
      </xsl:if>
      <xsl:variable name="self">
        <xsl:element name="o">
          <xsl:apply-templates select="@* except @name"/>
          <xsl:copy-of select="o[position()!=last()]"/>
        </xsl:element>
      </xsl:variable>
      <xsl:apply-templates select="$self"/>
      <xsl:apply-templates select="o[last()]"/>
    </application>
  </xsl:template>
  <xsl:template match="o" mode="app">
    <xsl:choose>
      <xsl:when test="count(o)=1">
        <application>
          <xsl:if test="@name">
            <xsl:attribute name="name" select="@name"/>
          </xsl:if>
          <xsl:if test="o/@as">
            <xsl:attribute name="as" select="o/@as"/>
          </xsl:if>
          <xsl:copy>
            <xsl:apply-templates select="@* except @name"/>
          </xsl:copy>
          <xsl:apply-templates select="o"/>
        </application>
      </xsl:when>
      <xsl:otherwise>
        <application>
          <xsl:if test="@name">
            <xsl:attribute name="name" select="@name"/>
          </xsl:if>
          <xsl:if test="o[last()]/@as">
            <xsl:attribute name="as" select="o[last()]/@as"/>
          </xsl:if>
          <xsl:variable name="self">
            <xsl:element name="o">
              <xsl:apply-templates select="@* except @name"/>
              <xsl:copy-of select="o[position()!=last()]"/>
            </xsl:element>
          </xsl:variable>
          <xsl:apply-templates select="$self"/>
          <xsl:apply-templates select="o[last()]"/>
        </application>
      </xsl:otherwise>
    </xsl:choose>
  </xsl:template>
  <xsl:template match="node()|@*">
    <xsl:copy>
      <xsl:apply-templates select="node()|@*"/>
    </xsl:copy>
  </xsl:template>
</xsl:stylesheet>
