<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" id="cache" version="2.0">
  <xsl:output encoding="UTF-8" method="xml"/>
  <!-- DISPATCH -->
  <xsl:template match="(/object/*)|formation">
    <xsl:apply-templates select="." mode="cache"/>
  </xsl:template>
  <xsl:template match="*[name()!='formation']" mode="cache">
    <xsl:copy>
      <xsl:apply-templates select="@*"/>
      <xsl:attribute name="cache"/>
      <xsl:apply-templates select="node()"/>
    </xsl:copy>
  </xsl:template>
  <xsl:template match="formation" mode="cache">
    <xsl:copy>
      <xsl:apply-templates select="@*"/>
      <xsl:attribute name="cache"/>
      <xsl:apply-templates select="node()" mode="cache"/>
    </xsl:copy>
  </xsl:template>
  <xsl:template match="node()|@*">
    <xsl:copy>
      <xsl:apply-templates select="node()|@*"/>
    </xsl:copy>
  </xsl:template>
</xsl:stylesheet>
