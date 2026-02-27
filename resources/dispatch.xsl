<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" id="dispatch" version="2.0">
  <xsl:output encoding="UTF-8" method="xml"/>
  <!-- DISPATCH -->
  <xsl:template match="o[not(starts-with(@base, '.')) and contains(@base, '.')]">
    <xsl:variable name="parts" select="tokenize(@base, '\.')"/>
    <xsl:variable name="count" select="count($parts)"/>
    <xsl:if test="$count=2">
      <xsl:element name="dispatch">
        <xsl:if test="$parts[1]='Φ'">
          <xsl:attribute name="from" select="'0'"/>
        </xsl:if>
        <xsl:if test="$parts[1]='ξ'">
          <xsl:attribute name="from" select="'-1'"/>
        </xsl:if>
        <xsl:attribute name="attr" select="$parts[2]"/>
        <xsl:if test="@name">
          <xsl:attribute name="name" select="@name" />
        </xsl:if>
        <xsl:apply-templates select="*"/>
      </xsl:element>
    </xsl:if>
    <xsl:if test="$count&gt;2">
      <xsl:element name="dispatch">
        <xsl:attribute name="attr" select="$parts[last()]"/>
        <xsl:if test="@name">
          <xsl:attribute name="name" select="@name"/>
        </xsl:if>
        <xsl:variable name="next">
          <xsl:element name="o">
            <xsl:apply-templates select="@*[name()!='base' and name()!='name']"/>
            <xsl:attribute name="base" select="string-join($parts[position()&lt;last()], '.')"/>
          </xsl:element>
        </xsl:variable>
        <xsl:apply-templates select="$next"/>
        <xsl:apply-templates select="*"/>
      </xsl:element>
    </xsl:if>
  </xsl:template>
  <xsl:template match="o[starts-with(@base, '.')]">
    <xsl:element name="dispatch">
      <xsl:attribute name="attr" select="substring-after(@base, '.')"/>
      <xsl:if test="@name">
        <xsl:attribute name="name" select="@name"/>
      </xsl:if>
      <xsl:apply-templates select="*[position()=1]"/>
    </xsl:element>
  </xsl:template>
  <xsl:template match="o[@base='ξ']">
    <xsl:element name="dispatch">
      <xsl:attribute name="self"/>
      <xsl:if test="@name">
        <xsl:attribute name="name" select="@name"/>
      </xsl:if>
    </xsl:element>
  </xsl:template>
  <xsl:template match="node()|@*">
    <xsl:copy>
      <xsl:apply-templates select="node()|@*"/>
    </xsl:copy>
  </xsl:template>
</xsl:stylesheet>
