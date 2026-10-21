package vs_fla
{
   import adobe.utils.*;
   import flash.accessibility.*;
   import flash.desktop.*;
   import flash.display.*;
   import flash.errors.*;
   import flash.events.*;
   import flash.external.*;
   import flash.filters.*;
   import flash.geom.*;
   import flash.globalization.*;
   import flash.media.*;
   import flash.net.*;
   import flash.net.drm.*;
   import flash.printing.*;
   import flash.profiler.*;
   import flash.sampler.*;
   import flash.sensors.*;
   import flash.system.*;
   import flash.text.*;
   import flash.text.engine.*;
   import flash.text.ime.*;
   import flash.ui.*;
   import flash.utils.*;
   import flash.xml.*;
   
   public dynamic class victory_5 extends MovieClip
   {
      
      public var team2LastPlayers:SrvSectionStatusIndicator;
      
      public var team1LastPlayers:SrvSectionStatusIndicator;
      
      public var textScore:TextField;
      
      public var textField:TextField;
      
      public function victory_5()
      {
         super();
         addFrameScript(0,this.frame1);
         this.__setProp_team1LastPlayers_victory();
         this.__setProp_team2LastPlayers_victory();
      }
      
      internal function __setProp_team1LastPlayers_victory() : *
      {
         try
         {
            this.team1LastPlayers["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.team1LastPlayers.enabled = true;
         this.team1LastPlayers.enableInitCallback = false;
         this.team1LastPlayers.maximum = 16;
         this.team1LastPlayers.minimum = 0;
         this.team1LastPlayers.value = 0;
         this.team1LastPlayers.visible = false;
         try
         {
            this.team1LastPlayers["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_team2LastPlayers_victory() : *
      {
         try
         {
            this.team2LastPlayers["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.team2LastPlayers.enabled = true;
         this.team2LastPlayers.enableInitCallback = false;
         this.team2LastPlayers.maximum = 16;
         this.team2LastPlayers.minimum = 0;
         this.team2LastPlayers.value = 0;
         this.team2LastPlayers.visible = false;
         try
         {
            this.team2LastPlayers["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function frame1() : *
      {
         stop();
         this.team1LastPlayers.registerRenderer(WhiteDead);
         this.team2LastPlayers.registerRenderer(RedDead);
      }
   }
}

