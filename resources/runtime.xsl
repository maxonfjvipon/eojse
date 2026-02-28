<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" id="runtime" version="2.0">
  <xsl:output encoding="UTF-8" method="xml"/>
  <!-- FORMATION -->
  <xsl:template match="object">
    <xsl:copy>
      <xsl:apply-templates select="node()|@*"/>
      <o name="bytes">
        <o base="∅" name="data"/>
        <o base="ξ.data" name="φ"/>
      </o>
      <o name="number">
        <o base="∅" name="as-bytes"/>
        <o base="ξ" name="xi"/>
        <o base="ξ.as-bytes" name="φ"/>
        <o name="plus">
          <o base="∅" name="x"/>
          <o name="λ"/>
        </o>
        <o name="times">
          <o base="∅" name="x"/>
          <o name="λ"/>
        </o>
        <o base="ξ.times" name="neg">
          <o as="α0" base="Φ.number">
            <o as="α0" base="Φ.bytes">
              <o as="α0">BF-F0-00-00-00-00-00-00</o>
            </o>
          </o>
        </o>
        <o name="minus">
          <o base="∅" name="x"/>
          <o base="ξ.ρ.plus" name="φ">
            <o as="α0" base="ξ.x.neg"/>
          </o>
        </o>
        <o name="gt">
          <o base="∅" name="x"/>
          <o name="λ"/>
        </o>
      </o>
      <o name="true">
        <o base="Φ.bytes" name="φ">
          <o as="α0">01-</o>
        </o>
        <o line="16" name="if" pos="2">
          <o base="∅" name="left"/>
          <o base="∅" name="right"/>
          <o base="ξ.left" name="φ"/>
        </o>
      </o>
      <o name="false">
        <o base="Φ.bytes" name="φ">
          <o as="α0">00-</o>
        </o>
        <o line="16" name="if" pos="2">
          <o base="∅" name="left"/>
          <o base="∅" name="right"/>
          <o base="ξ.right" name="φ"/>
        </o>
      </o>
      <o base="Φ.program" name="φ"/>
    </xsl:copy>
  </xsl:template>
  <xsl:template match="node()|@*">
    <xsl:copy>
      <xsl:apply-templates select="node()|@*"/>
    </xsl:copy>
  </xsl:template>
</xsl:stylesheet>